//! Part 9 of `docs/ipc.md`: exporting an instance as a Modrinth `.mrpack`.
//!
//! What goes where (the policy, in one place):
//!
//! * Content files (mods, resource packs, shader packs, datapacks) are looked
//!   up on Modrinth by SHA-1, in one batch call. A file Modrinth hosts becomes
//!   an entry of `files` with its hashes, size and Modrinth's download URL, so
//!   nothing is redistributed. This covers records installed from Modrinth
//!   and local jars (a pack imported earlier, say).
//! * A file Modrinth does not host goes into `overrides/` when it is a local
//!   jar (records written by older versions for other sources count as local
//!   jars) or a Modrinth record changed since it was installed.
//! * `includeConfig` adds `config/`, `defaultconfigs/`, `kubejs/`,
//!   `scripts/`, `options.txt` (without its `lastServer:` line),
//!   `optionsof.txt`, `optionsshaders.txt` and the shader packs' `*.txt`
//!   settings. `servers.dat` is never exported (it lists the user's
//!   servers).
//! * Never exported, by construction (only the folders above are read):
//!   accounts, tokens, logs, crash reports, saves, screenshots, backups,
//!   `.mangoclient-*` files and the MangoConfig jar.
//! * `icon.png` at the root of the archive is the instance's image icon
//!   (an extra file, ignored by other launchers; our importer reads it).
//!
//! The limits are the importer's own, so every pack written here can be read
//! back.

use crate::backups::zip_name_ok;
use crate::error::{AppError, Res};
use crate::outfile::{add_bytes, add_file, write_zip_atomic, FileExport};
use crate::service::AppService;
use mango_archive::Limits;
use mango_core::{Loader, ModRecord, ModType, Profile};
use mango_modrinth as modrinth;
use serde::Deserialize;
use serde_json::{json, Value};
use sha1::Sha1;
use sha2::{Digest, Sha512};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use ts_rs::TS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, serde::Serialize, TS)]
#[serde(rename_all = "lowercase")]
pub enum ExportFormat {
    Mrpack,
}

/// Biggest settings file or folder entry taken with `includeConfig`.
const MAX_CONFIG_FILE: u64 = 32 * 1024 * 1024;
const MAX_WALK_DEPTH: usize = 16;
const CONFIG_DIRS: [&str; 4] = ["config", "defaultconfigs", "kubejs", "scripts"];
const CONFIG_FILES: [&str; 3] = ["options.txt", "optionsof.txt", "optionsshaders.txt"];
/// Printed warnings are capped; the rest only count.
const MAX_WARNINGS: usize = 25;

fn too_large() -> AppError {
    AppError::new("export.too_large", "The pack would be too large to export")
}

/// A content file of the instance with its hashes.
struct Item {
    /// Path inside the instance, `/`-separated.
    rel: String,
    abs: PathBuf,
    size: u64,
    sha1: String,
    sha512: String,
    /// What to call it in a warning.
    label: String,
    /// A record installed from Modrinth.
    from_modrinth: bool,
}

enum Source {
    File(PathBuf),
    Bytes(Vec<u8>),
}

struct Override {
    rel: String,
    len: u64,
    source: Source,
}

struct Gathered {
    items: Vec<Item>,
    config: Vec<Override>,
    warnings: Vec<String>,
}

fn folder_of(kind: ModType) -> &'static str {
    match kind {
        ModType::Mod => "mods",
        ModType::Shader => "shaderpacks",
        ModType::Resourcepack => "resourcepacks",
        ModType::Datapack => "datapacks",
    }
}

fn label_of(rec: &ModRecord) -> String {
    rec.title
        .clone()
        .filter(|t| !t.is_empty())
        .or_else(|| rec.filename.clone())
        .unwrap_or_else(|| "A file".to_string())
}

/// `<instance>/a/b` -> `a/b` when it is a regular file really inside the
/// instance (no link anywhere on the way) and has a name a zip can hold.
fn rel_if_inside(instance: &Path, real_instance: &Path, abs: &Path) -> Option<String> {
    let meta = fs::symlink_metadata(abs).ok()?;
    if !meta.is_file() {
        return None;
    }
    if !fs::canonicalize(abs).ok()?.starts_with(real_instance) {
        return None;
    }
    let rel = abs.strip_prefix(instance).ok()?;
    let parts: Vec<&str> = rel
        .components()
        .map(|c| c.as_os_str().to_str())
        .collect::<Option<_>>()?;
    let rel = parts.join("/");
    zip_name_ok(&rel).then_some(rel)
}

/// The file behind a record: its stored path when that is inside the
/// instance, else `<folder>/<filename>` (also with `.disabled`).
fn locate(instance: &Path, real_instance: &Path, rec: &ModRecord) -> Option<(PathBuf, String)> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Some(rel) = rec
        .file
        .as_deref()
        .and_then(|f| Path::new(f).strip_prefix(instance).ok())
    {
        candidates.push(instance.join(rel));
    }
    if let Some(name) = rec
        .filename
        .as_deref()
        .filter(|n| crate::media::bare_name(n))
    {
        let dir = instance.join(folder_of(rec.kind.unwrap_or(ModType::Mod)));
        candidates.push(dir.join(name));
        candidates.push(dir.join(format!("{name}.disabled")));
    }
    candidates.into_iter().find_map(|abs| {
        let rel = rel_if_inside(instance, real_instance, &abs)?;
        Some((abs, rel))
    })
}

fn hash_file(path: &Path) -> std::io::Result<(String, String, u64)> {
    let mut file = fs::File::open(path)?;
    let (mut sha1, mut sha512) = (Sha1::new(), Sha512::new());
    let mut buf = vec![0u8; 64 * 1024];
    let mut size = 0u64;
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        sha1.update(&buf[..n]);
        sha512.update(&buf[..n]);
        size += n as u64;
    }
    Ok((
        hex::encode(sha1.finalize()),
        hex::encode(sha512.finalize()),
        size,
    ))
}

/// Everything below `dir` that is a regular file with a usable name; links
/// are never followed.
fn walk(
    dir: &Path,
    rel: &str,
    depth: usize,
    out: &mut Vec<(String, PathBuf, u64)>,
    warnings: &mut Vec<String>,
) {
    let Ok(rd) = fs::read_dir(dir) else { return };
    let mut entries: Vec<_> = rd.flatten().collect();
    entries.sort_by_key(fs::DirEntry::file_name);
    for e in entries {
        let Some(name) = e.file_name().to_str().map(str::to_string) else {
            warnings.push(format!("Skipped a file with an unreadable name in {rel}"));
            continue;
        };
        let child = format!("{rel}/{name}");
        let Ok(kind) = e.file_type() else { continue };
        if kind.is_dir() {
            if depth < MAX_WALK_DEPTH {
                walk(&e.path(), &child, depth + 1, out, warnings);
            }
        } else if kind.is_file() && zip_name_ok(&child) {
            if let Ok(meta) = e.metadata() {
                out.push((child, e.path(), meta.len()));
            }
        }
    }
}

fn is_private_config(rel: &str) -> bool {
    let lower = rel.to_ascii_lowercase();
    let name = lower.rsplit('/').next().unwrap_or("");
    name.contains("mangoconfig") || name.ends_with(".log") || name.starts_with(".mangoclient-")
}

/// Blocking part: find, hash and classify what is in the instance.
fn gather(paths: &mango_core::Paths, profile: &Profile, include_config: bool) -> Res<Gathered> {
    let instance = paths.instance_dir(&profile.id)?;
    let real_instance = fs::canonicalize(&instance).unwrap_or_else(|_| instance.clone());
    let mut warnings = Vec::new();
    let mut items = Vec::new();
    let mut seen = HashSet::new();
    for rec in &profile.mods {
        let file_name = rec.filename.clone().unwrap_or_default();
        if mango_content::is_own_mango_jar(&mango_content::jar_name(&file_name)) {
            continue;
        }
        let label = label_of(rec);
        let Some((abs, rel)) = locate(&instance, &real_instance, rec) else {
            warnings.push(format!("{label}: the file was not found and is left out"));
            continue;
        };
        if !seen.insert(rel.clone()) {
            continue;
        }
        let (sha1, sha512, size) = hash_file(&abs)?;
        items.push(Item {
            rel,
            abs,
            size,
            sha1,
            sha512,
            label,
            from_modrinth: rec.local != Some(true)
                && !crate::content::has_foreign_provider(rec)
                && rec
                    .project_id
                    .as_deref()
                    .is_some_and(|p| !p.starts_with("local:")),
        });
    }

    // Resource and shader packs the launcher does not track are the player's
    // own; they take the same road as the tracked content.
    for folder in ["resourcepacks", "shaderpacks"] {
        let dir = instance.join(folder);
        let Ok(rd) = fs::read_dir(&dir) else { continue };
        let mut found: Vec<_> = rd.flatten().collect();
        found.sort_by_key(fs::DirEntry::file_name);
        for e in found {
            let Some(name) = e.file_name().to_str().map(str::to_string) else {
                continue;
            };
            // `<pack>.zip.txt` holds shader settings: configuration.
            if name.to_ascii_lowercase().ends_with(".txt") {
                continue;
            }
            let rel = format!("{folder}/{name}");
            if !zip_name_ok(&rel) || seen.contains(&rel) {
                continue;
            }
            let abs = e.path();
            if rel_if_inside(&instance, &real_instance, &abs).is_none() {
                continue;
            }
            let (sha1, sha512, size) = hash_file(&abs)?;
            seen.insert(rel.clone());
            items.push(Item {
                label: name,
                rel,
                abs,
                size,
                sha1,
                sha512,
                from_modrinth: false,
            });
        }
    }

    let mut config = Vec::new();
    if include_config {
        let mut files: Vec<(String, PathBuf, u64)> = Vec::new();
        for dir in CONFIG_DIRS {
            let abs = instance.join(dir);
            if fs::symlink_metadata(&abs).is_ok_and(|m| m.is_dir())
                && fs::canonicalize(&abs).is_ok_and(|a| a.starts_with(&real_instance))
            {
                walk(&abs, dir, 1, &mut files, &mut warnings);
            }
        }
        // Shader settings sit next to the packs: `<pack>.zip.txt`.
        let shaders = instance.join("shaderpacks");
        if fs::symlink_metadata(&shaders).is_ok_and(|m| m.is_dir()) {
            let mut all = Vec::new();
            walk(
                &shaders,
                "shaderpacks",
                MAX_WALK_DEPTH,
                &mut all,
                &mut warnings,
            );
            files.extend(all.into_iter().filter(|(rel, _, _)| rel.ends_with(".txt")));
        }
        for name in CONFIG_FILES {
            let abs = instance.join(name);
            if fs::symlink_metadata(&abs).is_ok_and(|m| m.is_file()) {
                if name == "options.txt" {
                    if let Some(bytes) = options_without_private_lines(&abs) {
                        config.push(Override {
                            rel: name.to_string(),
                            len: bytes.len() as u64,
                            source: Source::Bytes(bytes),
                        });
                    }
                    continue;
                }
                let len = fs::metadata(&abs)?.len();
                files.push((name.to_string(), abs, len));
            }
        }
        for (rel, abs, len) in files {
            if is_private_config(&rel) {
                continue;
            }
            if len > MAX_CONFIG_FILE {
                warnings.push(format!("{rel} is too large and was left out"));
                continue;
            }
            config.push(Override {
                rel,
                len,
                source: Source::File(abs),
            });
        }
    }
    Ok(Gathered {
        items,
        config,
        warnings,
    })
}

/// `options.txt` without the address of the last server played.
fn options_without_private_lines(path: &Path) -> Option<Vec<u8>> {
    let meta = fs::symlink_metadata(path).ok()?;
    if !meta.is_file() || meta.len() > 1024 * 1024 {
        return None;
    }
    let text = String::from_utf8_lossy(&fs::read(path).ok()?).into_owned();
    let kept: Vec<&str> = text
        .lines()
        .filter(|l| !l.starts_with("lastServer:"))
        .collect();
    let mut out = kept.join("\n");
    out.push('\n');
    Some(out.into_bytes())
}

fn loader_key(loader: Loader) -> Option<&'static str> {
    match loader {
        Loader::Vanilla => None,
        Loader::Fabric => Some("fabric-loader"),
        Loader::Quilt => Some("quilt-loader"),
        Loader::Forge => Some("forge"),
        Loader::Neoforge => Some("neoforge"),
    }
}

impl AppService {
    /// The loader version to pin: the profile's, else the newest stable one
    /// (a profile that was never launched has none stored).
    async fn export_loader_version(&self, profile: &Profile) -> Res<Option<String>> {
        if profile.loader == Loader::Vanilla {
            return Ok(None);
        }
        if !profile.loader_version.is_empty() {
            return Ok(Some(profile.loader_version.clone()));
        }
        let versions = self
            .loader_versions(
                modrinth::loader_name(profile.loader).to_string(),
                profile.mc_version.clone(),
            )
            .await
            .unwrap_or_default();
        versions
            .iter()
            .find(|v| v.stable)
            .or(versions.first())
            .map(|v| Some(v.version.clone()))
            .ok_or_else(|| {
                AppError::new(
                    "export.loader_unresolved",
                    "The loader version of this instance is not known yet; launch it once first",
                )
            })
    }

    /// Write the instance as a `.mrpack` (see the module docs for what goes
    /// where). `out_path` must be absolute, in an existing folder.
    pub async fn instance_export(
        &self,
        profile_id: String,
        format: ExportFormat,
        include_config: bool,
        out_path: String,
    ) -> Res<FileExport> {
        let ExportFormat::Mrpack = format;
        let profile = self.profile(&profile_id)?;
        let instance = self.inner.paths.instance_dir(&profile.id)?;
        let out = self.checked_out_path(&out_path, &[&instance])?;

        // Reconcile the list with the folder first, like `content_list`.
        let paths = self.inner.paths.clone();
        let snapshot = profile.clone();
        let synced = tokio::task::spawn_blocking(move || {
            mango_content::sync_profile_mods(&paths, &snapshot)
        })
        .await??;
        if synced.changed {
            self.save_mods(&profile.id, &synced.mods)?;
        }
        let mut profile = profile;
        profile.mods = synced.mods;

        let loader_version = self.export_loader_version(&profile).await?;

        let (paths, p2) = (self.inner.paths.clone(), profile.clone());
        let mut gathered =
            tokio::task::spawn_blocking(move || gather(&paths, &p2, include_config)).await??;

        // One batch lookup for everything that could be on Modrinth.
        let hashes: Vec<String> = gathered.items.iter().map(|i| i.sha1.clone()).collect();
        let found = if hashes.is_empty() {
            Ok(HashMap::new())
        } else {
            self.inner.modrinth.versions_by_sha1(&hashes).await
        };
        let found = match found {
            Ok(f) => f,
            Err(e) if gathered.items.iter().any(|i| i.from_modrinth) => {
                return Err(AppError::new(
                    "export.lookup_failed",
                    format!("Could not look the mods up on Modrinth: {e}"),
                ));
            }
            Err(_) => {
                gathered.warnings.push(
                    "Modrinth could not be reached; local files are included directly".into(),
                );
                HashMap::new()
            }
        };

        let mut files: Vec<Value> = Vec::new();
        let mut overrides: Vec<Override> = Vec::new();
        for item in std::mem::take(&mut gathered.items) {
            let hosted = found
                .get(&item.sha1)
                .and_then(|v| {
                    v.files
                        .iter()
                        .find(|f| {
                            f.hashes
                                .sha1
                                .as_deref()
                                .is_some_and(|h| h.eq_ignore_ascii_case(&item.sha1))
                        })
                        .map(|f| f.url.clone())
                })
                .filter(|u| !u.is_empty());
            match hosted {
                Some(url) => files.push(json!({
                    "path": item.rel,
                    "hashes": {"sha1": item.sha1, "sha512": item.sha512},
                    "env": {"client": "required", "server": "required"},
                    "downloads": [url],
                    "fileSize": item.size,
                })),
                None => {
                    if item.from_modrinth {
                        gathered.warnings.push(format!(
                            "{} differs from the Modrinth file and is included directly",
                            item.label
                        ));
                    }
                    overrides.push(Override {
                        rel: item.rel,
                        len: item.size,
                        source: Source::File(item.abs),
                    });
                }
            }
        }
        files.sort_by(|a, b| a["path"].as_str().cmp(&b["path"].as_str()));
        overrides.extend(gathered.config);
        overrides.sort_by(|a, b| a.rel.cmp(&b.rel));
        overrides.dedup_by(|a, b| a.rel == b.rel);

        let limits = Limits::default();
        let mut total = 0u64;
        if overrides.len() + 2 > limits.entries {
            return Err(too_large());
        }
        for o in &overrides {
            if o.len > limits.entry_size {
                return Err(too_large());
            }
            total = total.saturating_add(o.len);
            if total > limits.total_size {
                return Err(too_large());
            }
        }

        let mut dependencies = serde_json::Map::new();
        dependencies.insert("minecraft".into(), json!(profile.mc_version));
        if let (Some(key), Some(v)) = (loader_key(profile.loader), &loader_version) {
            dependencies.insert(key.into(), json!(v));
        }
        let index = json!({
            "formatVersion": 1,
            "game": "minecraft",
            "versionId": "1.0.0",
            "name": profile.name,
            "files": files,
            "dependencies": dependencies,
        });
        let index = serde_json::to_vec_pretty(&index)?;
        let icon = self
            .inner
            .paths
            .cover_file(&profile.id)
            .ok()
            .and_then(|f| fs::read(f).ok())
            .filter(|b| b.len() <= 5 * 1024 * 1024);

        let out2 = out.clone();
        let size = tokio::task::spawn_blocking(move || {
            write_zip_atomic(&out2, |zw| {
                add_bytes(zw, "modrinth.index.json", &index)?;
                if let Some(icon) = &icon {
                    add_bytes(zw, "icon.png", icon)?;
                }
                for o in &overrides {
                    let name = format!("overrides/{}", o.rel);
                    match &o.source {
                        Source::File(abs) => add_file(zw, &name, abs, o.len)?,
                        Source::Bytes(b) => add_bytes(zw, &name, b)?,
                    }
                }
                Ok(())
            })
        })
        .await??;

        let hidden = gathered.warnings.len().saturating_sub(MAX_WARNINGS);
        for line in gathered.warnings.iter().take(MAX_WARNINGS) {
            self.warn(&profile.id, line.clone());
        }
        if hidden > 0 {
            gathered.warnings.truncate(MAX_WARNINGS);
            gathered.warnings.push(format!("... and {hidden} more"));
        }
        Ok(FileExport {
            path: out.display().to_string(),
            size_bytes: size,
            warnings: gathered.warnings,
        })
    }
}
