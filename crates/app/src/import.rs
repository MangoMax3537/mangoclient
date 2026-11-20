//! Part 9 of `docs/ipc.md`: importing an instance from a pack file.
//!
//! The kind is detected from the archive's content, never from its name:
//!
//! * Modrinth `.mrpack`: `modrinth.index.json` at the root,
//! * CurseForge zip: `manifest.json` whose `manifestType` is `minecraftModpack`,
//! * Prism / MultiMC instance zip: `instance.cfg` + `mmc-pack.json` (at the
//!   root or inside one top-level folder); `.minecraft/` or `minecraft/` is
//!   the overrides folder.
//!
//! The first two go through the same code as installing a pack from the
//! provider (`finish_modrinth_info`, `install_curse_pack`), so downloads,
//! hash checks, the safe extraction of overrides, blocked CurseForge files
//! and the Modrinth fallback behave identically.

use crate::content::InstallReporter;
use crate::error::{AppError, Res};
use crate::install::CursePack;
use crate::models::{BlockedFile, ModpackPlan, Provider};
use crate::service::AppService;
use crate::types::ProfileView;
use mango_archive::{extract_zip_subtree, scan_zip_subtree, ArchiveError, Limits};
use mango_core::validation::is_safe_version;
use mango_core::{Loader, Profile};
use mango_modrinth as modrinth;
use serde::Serialize;
use serde_json::{json, Value};
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use ts_rs::TS;

/// Largest pack file accepted (the path comes from a dialog or a drop).
pub const MAX_IMPORT_BYTES: u64 = 2 * 1024 * 1024 * 1024;
/// Largest manifest / config entry read into memory.
const MAX_META_BYTES: u64 = 4 * 1024 * 1024;
/// Largest pack icon.
const MAX_ICON_BYTES: u64 = 5 * 1024 * 1024;
const MRPACK_INDEX: &str = "modrinth.index.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "lowercase")]
pub enum ImportKind {
    Mrpack,
    Curseforge,
    Prism,
}

/// What importing a file would create.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ImportPlan {
    pub kind: ImportKind,
    pub name: String,
    pub mc_version: String,
    pub loader: Loader,
    pub loader_version: Option<String>,
    /// Files the import puts in the instance (downloads plus, for a Prism
    /// instance, its files).
    pub file_count: u64,
    pub size_bytes: Option<u64>,
    /// CurseForge files that need the user (see `docs/ipc.md` Part 5).
    pub blocked_files: Vec<BlockedFile>,
}

fn invalid(msg: impl std::fmt::Display) -> AppError {
    AppError::new("import.invalid_pack", format!("Not a usable pack: {msg}"))
}

fn archive_err(e: ArchiveError) -> AppError {
    AppError::new("import.unsafe_archive", e.to_string())
}

/// A regular file (never a link) of a sane size, from an absolute path.
fn validate_import_path(raw: &str) -> Res<PathBuf> {
    if raw.is_empty() || raw.len() > 4096 || raw.contains('\0') {
        return Err(AppError::new("import.not_a_file", "Invalid file path"));
    }
    let path = PathBuf::from(raw);
    if !path.is_absolute() {
        return Err(AppError::new(
            "import.not_a_file",
            "The file path must be absolute",
        ));
    }
    let meta = std::fs::symlink_metadata(&path)
        .map_err(|_| AppError::new("import.not_found", "The file does not exist"))?;
    if !meta.is_file() {
        return Err(AppError::new(
            "import.not_a_file",
            "Only regular files can be imported",
        ));
    }
    if meta.len() > MAX_IMPORT_BYTES {
        return Err(AppError::new(
            "import.too_large",
            "The file is larger than 2 GB",
        ));
    }
    Ok(path)
}

fn open_zip(path: &Path) -> Res<zip::ZipArchive<File>> {
    let file = File::open(path)
        .map_err(|_| AppError::new("import.not_found", "The file cannot be opened"))?;
    // The opened handle cannot be a link: check what we actually got.
    let meta = file.metadata()?;
    if !meta.is_file() || meta.len() > MAX_IMPORT_BYTES {
        return Err(AppError::new(
            "import.not_a_file",
            "Only regular files can be imported",
        ));
    }
    let zip = zip::ZipArchive::new(file)
        .map_err(|_| AppError::new("import.not_an_archive", "The file is not a zip archive"))?;
    if zip.len() > Limits::default().entries {
        return Err(invalid("too many entries"));
    }
    Ok(zip)
}

/// One entry, at most `max` bytes (`None` when absent, too large or unreadable).
fn read_entry(zip: &mut zip::ZipArchive<File>, name: &str, max: u64) -> Option<Vec<u8>> {
    let entry = zip.by_name(name).ok()?;
    if entry.size() > max {
        return None;
    }
    let mut buf = Vec::new();
    entry.take(max + 1).read_to_end(&mut buf).ok()?;
    (buf.len() as u64 <= max).then_some(buf)
}

// ---------------------------------------------------------------- Prism

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PrismInfo {
    pub name: Option<String>,
    pub mc_version: String,
    pub loader: Loader,
    pub loader_version: String,
    /// `MaxMemAlloc` when the instance overrides the memory.
    pub ram: Option<u32>,
}

/// `key=value` lines of an `instance.cfg` (sections, comments and a BOM are
/// ignored; the last assignment of a key wins).
fn parse_cfg(text: &str) -> std::collections::HashMap<String, String> {
    let mut out = std::collections::HashMap::new();
    for line in text.trim_start_matches('\u{feff}').lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with(['#', ';', '[']) {
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            out.insert(k.trim().to_string(), v.trim().to_string());
        }
    }
    out
}

/// Name, Minecraft version, loader and memory of a Prism/MultiMC instance
/// from its `instance.cfg` and `mmc-pack.json`.
pub(crate) fn parse_prism(cfg: &str, pack: &[u8]) -> Res<PrismInfo> {
    let cfg = parse_cfg(cfg);
    let pack: Value =
        serde_json::from_slice(pack).map_err(|e| invalid(format!("mmc-pack.json: {e}")))?;
    let components = pack
        .get("components")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("mmc-pack.json has no components"))?;
    let version_of = |uid: &str| {
        components
            .iter()
            .find(|c| c.get("uid").and_then(Value::as_str) == Some(uid))
            .and_then(|c| c.get("version").and_then(Value::as_str))
            .map(str::trim)
            .filter(|v| !v.is_empty())
    };
    let mc_version = version_of("net.minecraft")
        .ok_or_else(|| invalid("the instance names no Minecraft version"))?
        .to_string();
    let (loader, raw) = [
        ("org.quiltmc.quilt-loader", Loader::Quilt),
        ("net.neoforged", Loader::Neoforge),
        ("net.minecraftforge", Loader::Forge),
        ("net.fabricmc.fabric-loader", Loader::Fabric),
    ]
    .into_iter()
    .find_map(|(uid, l)| version_of(uid).map(|v| (l, v)))
    .unwrap_or((Loader::Vanilla, ""));
    // Older Forge components carry the game version: `1.20.1-47.2.0`.
    let loader_version = raw
        .strip_prefix(&format!("{mc_version}-"))
        .unwrap_or(raw)
        .to_string();
    check_versions(&mc_version, &loader_version)?;
    let ram = (cfg.get("OverrideMemory").map(String::as_str) == Some("true"))
        .then(|| cfg.get("MaxMemAlloc").and_then(|v| v.parse::<u32>().ok()))
        .flatten()
        .filter(|r| (1024..=131_072).contains(r));
    Ok(PrismInfo {
        name: cfg.get("name").filter(|n| !n.is_empty()).cloned(),
        mc_version,
        loader,
        loader_version,
        ram,
    })
}

fn check_versions(mc: &str, loader_version: &str) -> Res<()> {
    if !is_safe_version(mc) {
        return Err(invalid("unsupported Minecraft version"));
    }
    if !loader_version.is_empty() && !is_safe_version(loader_version) {
        return Err(invalid("unsupported loader version"));
    }
    Ok(())
}

/// Folder (relative path inside the zip, with trailing `/`) holding the
/// instance's `instance.cfg`: the root or one top-level folder.
fn prism_root(names: &[String]) -> Option<String> {
    let mut roots: Vec<String> = Vec::new();
    for n in names {
        let dir = match n.rsplit_once('/') {
            None if n == "instance.cfg" => String::new(),
            Some((d, f)) if f == "instance.cfg" && !d.contains('/') => format!("{d}/"),
            _ => continue,
        };
        roots.push(dir);
    }
    roots.sort_by_key(String::len);
    roots
        .into_iter()
        .find(|r| names.iter().any(|n| *n == format!("{r}mmc-pack.json")))
}

/// What the launcher's own work must never be overwritten with.
fn prism_skip(rel: &str) -> bool {
    const SKIP: [&str; 4] = ["logs/", "crash-reports/", ".fabric/", ".mixin.out/"];
    mango_archive::is_launcher_private(rel) || SKIP.iter().any(|p| rel.starts_with(p))
}

// ---------------------------------------------------------------- detection

enum Scanned {
    Mrpack(modrinth::PackSummary),
    Curseforge(mango_curseforge::Manifest),
    Prism {
        info: PrismInfo,
        /// Prefix of the overrides folder inside the zip, if any.
        overrides: Option<String>,
        files: usize,
        bytes: u64,
    },
}

struct Scan {
    what: Scanned,
    icon: Option<Vec<u8>>,
}

/// Blocking: classify the archive and validate what can be validated without
/// the network.
fn scan_archive(path: &Path) -> Res<Scan> {
    let mut zip = open_zip(path)?;
    let names: Vec<String> = zip.file_names().map(str::to_string).collect();
    let has = |n: &str| names.iter().any(|x| x == n);
    // Nothing is extracted here, so a root that never exists is enough to
    // check that the paths stay inside.
    let scratch = path.with_extension("import-scratch-nonexistent");

    if has(MRPACK_INDEX) {
        let summary = modrinth::inspect_pack(path, &scratch)?;
        let icon = read_entry(&mut zip, "icon.png", MAX_ICON_BYTES);
        return Ok(Scan {
            what: Scanned::Mrpack(summary),
            icon,
        });
    }
    if has("manifest.json") {
        let is_curse = read_entry(&mut zip, "manifest.json", MAX_META_BYTES)
            .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
            .is_some_and(|v| {
                v.get("manifestType").and_then(Value::as_str) == Some("minecraftModpack")
            });
        if is_curse {
            let manifest = mango_curseforge::read_manifest(path)?;
            scan_zip_subtree(
                File::open(path)?,
                &format!("{}/", manifest.overrides),
                &scratch,
                &Limits::default(),
                &|_| false,
            )
            .map_err(archive_err)?;
            let icon = read_entry(&mut zip, "icon.png", MAX_ICON_BYTES);
            return Ok(Scan {
                what: Scanned::Curseforge(manifest),
                icon,
            });
        }
    }
    if let Some(root) = prism_root(&names) {
        let cfg = read_entry(&mut zip, &format!("{root}instance.cfg"), MAX_META_BYTES)
            .ok_or_else(|| invalid("instance.cfg is unreadable"))?;
        let pack = read_entry(&mut zip, &format!("{root}mmc-pack.json"), MAX_META_BYTES)
            .ok_or_else(|| invalid("mmc-pack.json is unreadable"))?;
        let info = parse_prism(&String::from_utf8_lossy(&cfg), &pack)?;
        let overrides = [".minecraft/", "minecraft/"]
            .into_iter()
            .map(|d| format!("{root}{d}"))
            .find(|p| names.iter().any(|n| n.starts_with(p.as_str())));
        let (files, bytes) = match &overrides {
            Some(prefix) => scan_zip_subtree(
                File::open(path)?,
                prefix,
                &scratch,
                &Limits::default(),
                &prism_skip,
            )
            .map_err(archive_err)?,
            None => (0, 0),
        };
        let icon = read_entry(&mut zip, &format!("{root}icon.png"), MAX_ICON_BYTES);
        return Ok(Scan {
            what: Scanned::Prism {
                info,
                overrides,
                files,
                bytes,
            },
            icon,
        });
    }
    Err(AppError::new(
        "import.unsupported_format",
        "This archive is not a Modrinth, CurseForge or Prism/MultiMC pack",
    ))
}

enum How {
    Mrpack,
    Curse(Box<CursePack>),
    Prism {
        info: PrismInfo,
        overrides: Option<String>,
    },
}

struct Prepared {
    path: PathBuf,
    plan: ImportPlan,
    how: How,
    icon: Option<Vec<u8>>,
}

fn file_stem(path: &Path) -> String {
    path.file_stem()
        .map(|s| s.to_string_lossy().trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "Imported pack".to_string())
}

/// A profile name: no control characters, at most 128 characters.
fn clean_name(raw: &str) -> String {
    raw.chars()
        .filter(|c| !c.is_control())
        .collect::<String>()
        .trim()
        .chars()
        .take(128)
        .collect::<String>()
        .trim()
        .to_string()
}

impl AppService {
    async fn prepare_import(&self, file_path: &str) -> Res<Prepared> {
        let path = validate_import_path(file_path)?;
        let p = path.clone();
        let scan = tokio::task::spawn_blocking(move || scan_archive(&p)).await??;
        let stem = file_stem(&path);
        let (plan, how) = match scan.what {
            Scanned::Mrpack(summary) => {
                let info = summary.info;
                let mc = info
                    .mc_version
                    .filter(|m| !m.is_empty())
                    .ok_or_else(|| invalid("the pack names no Minecraft version"))?;
                check_versions(&mc, &info.loader_version)?;
                let plan = ImportPlan {
                    kind: ImportKind::Mrpack,
                    name: info
                        .name
                        .map(|n| clean_name(&n))
                        .filter(|n| !n.is_empty())
                        .unwrap_or(stem),
                    mc_version: mc,
                    loader: info.loader,
                    loader_version: (!info.loader_version.is_empty())
                        .then_some(info.loader_version),
                    file_count: info.file_count as u64,
                    size_bytes: Some(summary.download_bytes),
                    blocked_files: Vec::new(),
                };
                (plan, How::Mrpack)
            }
            Scanned::Curseforge(manifest) => {
                check_versions(&manifest.mc_version, &manifest.loader_version)?;
                let client = self.curseforge_client()?;
                let (manifest, resolved) = Self::resolve_pack_zip(&client, &path).await?;
                let blocked = self.blocked_entries(&resolved).await;
                let name = manifest
                    .name
                    .as_deref()
                    .map(clean_name)
                    .filter(|n| !n.is_empty())
                    .unwrap_or(stem);
                let pack_plan = ModpackPlan {
                    provider: Provider::Curseforge,
                    project_id: String::new(),
                    version_id: String::new(),
                    name: name.clone(),
                    mc_version: manifest.mc_version.clone(),
                    loader: manifest.loader,
                    loader_version: (!manifest.loader_version.is_empty())
                        .then(|| manifest.loader_version.clone()),
                    file_count: (resolved.entries.len() + resolved.missing.len()) as u64,
                    size_bytes: Some(resolved.download_bytes()),
                    icon_url: None,
                    blocked_files: blocked.iter().map(|b| b.file.clone()).collect(),
                };
                let plan = ImportPlan {
                    kind: ImportKind::Curseforge,
                    name,
                    mc_version: pack_plan.mc_version.clone(),
                    loader: pack_plan.loader,
                    loader_version: pack_plan.loader_version.clone(),
                    file_count: pack_plan.file_count,
                    size_bytes: pack_plan.size_bytes,
                    blocked_files: pack_plan.blocked_files.clone(),
                };
                let pack = CursePack {
                    plan: pack_plan,
                    zip: path.clone(),
                    resolved,
                    blocked,
                };
                (plan, How::Curse(Box::new(pack)))
            }
            Scanned::Prism {
                info,
                overrides,
                files,
                bytes,
            } => {
                let plan = ImportPlan {
                    kind: ImportKind::Prism,
                    name: info
                        .name
                        .as_deref()
                        .map(clean_name)
                        .filter(|n| !n.is_empty())
                        .unwrap_or(stem),
                    mc_version: info.mc_version.clone(),
                    loader: info.loader,
                    loader_version: (!info.loader_version.is_empty())
                        .then(|| info.loader_version.clone()),
                    file_count: files as u64,
                    size_bytes: Some(bytes),
                    blocked_files: Vec::new(),
                };
                (plan, How::Prism { info, overrides })
            }
        };
        Ok(Prepared {
            path,
            plan,
            how,
            icon: scan.icon,
        })
    }

    /// What importing `file_path` would create. Reads the archive (and, for
    /// CurseForge, asks the API about its files); changes nothing.
    pub async fn instance_import_plan(&self, file_path: String) -> Res<ImportPlan> {
        self.checked_in_path(&file_path, crate::path_grants::GrantKind::File)?;
        Ok(self.prepare_import(&file_path).await?.plan)
    }

    /// Create a profile from the file. Progress goes out as
    /// `install:progress` for the new profile; a failure leaves no profile.
    pub async fn instance_import(
        &self,
        file_path: String,
        name: Option<String>,
    ) -> Res<ProfileView> {
        self.checked_in_path(&file_path, crate::path_grants::GrantKind::File)?;
        let mut prepared = self.prepare_import(&file_path).await?;
        let plan = prepared.plan.clone();
        let name = name
            .as_deref()
            .map(clean_name)
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| plan.name.clone());
        let mut data = json!({
            "name": name,
            "mcVersion": plan.mc_version,
            "loader": plan.loader,
            "loaderVersion": plan.loader_version.clone().unwrap_or_default(),
        });
        if let How::Prism { info, .. } = &prepared.how {
            if let Some(ram) = info.ram {
                data["ram"] = json!(ram);
            }
        }
        let created = self.inner.with_store(|s| s.add_profile(&data))?;
        match self.run_import(&created, &mut prepared).await {
            Ok(()) => {
                if let Some(icon) = prepared.icon.take() {
                    self.apply_pack_logo_bytes(&created.id, icon).await;
                }
                let fresh = self.profile(&created.id)?;
                Ok(crate::blocked::view_of(&self.inner.paths, &fresh))
            }
            Err(e) => {
                // A half-installed pack is worse than none.
                let _ = self.inner.with_store(|s| s.delete_profile(&created.id));
                Err(e)
            }
        }
    }

    async fn run_import(&self, profile: &Profile, prepared: &mut Prepared) -> Res<()> {
        let reporter = InstallReporter::new(&self.inner, &profile.id, "import");
        reporter.emit(0, 1, "import");
        match &mut prepared.how {
            How::Mrpack => {
                let progress: Arc<modrinth::ProgressFn> = {
                    let r = reporter.clone();
                    Arc::new(move |p: &modrinth::ModpackProgress| {
                        r.emit_throttled(p.done as u64, p.total as u64, &p.label);
                    })
                };
                let info = modrinth::install_pack_file(
                    &prepared.path,
                    &self.inner.paths,
                    profile,
                    Some(progress),
                    self.inner.modrinth.download_policy(),
                )
                .await?;
                self.finish_modrinth_info(profile, &reporter, info).await?;
            }
            How::Curse(pack) => {
                self.install_curse_pack(profile, &reporter, pack).await?;
            }
            How::Prism { overrides, .. } => {
                let game_dir = self.inner.paths.instance_dir(&profile.id)?;
                if let Some(prefix) = overrides.clone() {
                    let path = prepared.path.clone();
                    tokio::task::spawn_blocking(move || -> Res<()> {
                        extract_zip_subtree(
                            File::open(&path)?,
                            &prefix,
                            &game_dir,
                            &Limits::default(),
                            &prism_skip,
                        )
                        .map_err(archive_err)?;
                        Ok(())
                    })
                    .await??;
                }
                let paths = self.inner.paths.clone();
                let snapshot = self.profile(&profile.id)?;
                let synced = tokio::task::spawn_blocking(move || {
                    mango_content::sync_profile_mods(&paths, &snapshot)
                })
                .await??;
                self.save_mods(&profile.id, &synced.mods)?;
            }
        }
        reporter.emit(1, 1, "done");
        Ok(())
    }
}
