//! Modrinth modpack (`.mrpack`) installation.

use std::collections::HashMap;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use mango_archive::{safe_archive_path, validate_entries, EntryInfo, Limits};
use mango_core::{Loader, Paths, Profile};
use mango_net::{download_file, pool, DownloadOptions, Expected};
use serde::Deserialize;

use crate::client::{primary_file, Client};
use crate::error::{Error, Result};
use crate::install::{expected_for, safe_filename, LogFn};

pub const MAX_INDEX_BYTES: u64 = 4 * 1024 * 1024;
const INDEX_NAME: &str = "modrinth.index.json";
const OVERRIDE_FOLDERS: [&str; 2] = ["overrides", "client-overrides"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModpackProgress {
    pub done: usize,
    pub total: usize,
    pub label: String,
}

pub type ProgressFn = dyn Fn(&ModpackProgress) + Send + Sync;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModpackInfo {
    pub name: Option<String>,
    pub mc_version: Option<String>,
    pub loader: Loader,
    pub loader_version: String,
    pub file_count: usize,
}

#[derive(Debug, Default, Deserialize)]
struct PackHashes {
    sha1: Option<String>,
    sha512: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct PackEnv {
    client: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PackFile {
    #[serde(default)]
    path: String,
    #[serde(default)]
    hashes: PackHashes,
    #[serde(default)]
    downloads: Vec<String>,
    #[serde(default)]
    file_size: Option<u64>,
    #[serde(default)]
    env: Option<PackEnv>,
}

#[derive(Debug, Default, Deserialize)]
struct PackIndex {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    files: Vec<PackFile>,
    #[serde(default)]
    dependencies: HashMap<String, String>,
}

/// Loader and loader version from the pack's `dependencies` table.
pub fn detect_loader(deps: &HashMap<String, String>) -> (Loader, String) {
    for (key, loader) in [
        ("fabric-loader", Loader::Fabric),
        ("quilt-loader", Loader::Quilt),
        ("neoforge", Loader::Neoforge),
        ("forge", Loader::Forge),
    ] {
        if let Some(v) = deps.get(key).filter(|v| !v.is_empty()) {
            return (loader, v.clone());
        }
    }
    (Loader::Vanilla, String::new())
}

fn invalid(msg: &str) -> Error {
    Error::InvalidModpack(msg.into())
}

fn zip_err(e: zip::result::ZipError) -> Error {
    Error::InvalidModpack(e.to_string())
}

/// Validate the archive and read its index. Everything that could be unsafe
/// (entry count, symlinks, sizes, override paths) is checked here, before any
/// download starts.
fn read_pack(pack: &Path, game_dir: &Path) -> Result<PackIndex> {
    let limits = Limits::default();
    let file = fs::File::open(pack).map_err(|e| Error::io(pack, e))?;
    let mut zip = zip::ZipArchive::new(file).map_err(zip_err)?;
    if zip.len() > limits.entries {
        return Err(invalid("too many entries"));
    }
    let mut overrides = Vec::new();
    for i in 0..zip.len() {
        let f = zip.by_index_raw(i).map_err(zip_err)?;
        let name = f.name().to_string();
        if OVERRIDE_FOLDERS
            .iter()
            .any(|p| name.starts_with(&format!("{p}/")))
        {
            overrides.push(EntryInfo {
                name,
                size: f.size(),
                is_dir: f.is_dir(),
                unix_mode: f.unix_mode(),
            });
        }
    }
    validate_entries(overrides.iter().cloned(), &limits)?;
    for e in overrides.iter().filter(|e| !e.is_dir) {
        let rel = override_rel(&e.name);
        safe_archive_path(game_dir, rel)?;
    }

    let mut entry = zip
        .by_name(INDEX_NAME)
        .map_err(|_| invalid("modrinth.index.json is missing"))?;
    if entry.size() > MAX_INDEX_BYTES {
        return Err(invalid("index is too large"));
    }
    let mut buf = Vec::new();
    (&mut entry)
        .take(MAX_INDEX_BYTES + 1)
        .read_to_end(&mut buf)
        .map_err(|e| Error::io(pack, e))?;
    if buf.len() as u64 > MAX_INDEX_BYTES {
        return Err(invalid("index is too large"));
    }
    let index: PackIndex =
        serde_json::from_slice(&buf).map_err(|e| Error::InvalidModpack(e.to_string()))?;
    Ok(index)
}

fn override_rel(name: &str) -> &str {
    OVERRIDE_FOLDERS
        .iter()
        .find_map(|p| name.strip_prefix(&format!("{p}/")))
        .unwrap_or(name)
}

/// Copy `overrides/` then `client-overrides/` into the game dir. New files are
/// created exclusively; an existing target is overwritten only when it is a
/// regular file (never a symlink).
fn extract_overrides(pack: &Path, game_dir: &Path) -> Result<()> {
    let limits = Limits::default();
    let file = fs::File::open(pack).map_err(|e| Error::io(pack, e))?;
    let mut zip = zip::ZipArchive::new(file).map_err(zip_err)?;
    let mut total = 0u64;
    for folder in OVERRIDE_FOLDERS {
        let prefix = format!("{folder}/");
        for i in 0..zip.len() {
            let mut f = zip.by_index(i).map_err(zip_err)?;
            let name = f.name().to_string();
            if !name.starts_with(&prefix) || f.is_dir() {
                continue;
            }
            let rel = &name[prefix.len()..];
            let dest = safe_archive_path(game_dir, rel)?;
            if let Some(parent) = dest.parent() {
                fs::create_dir_all(parent).map_err(|e| Error::io(parent, e))?;
            }
            // Re-check now that directories exist (a link may have appeared).
            safe_archive_path(game_dir, rel)?;
            let mut out = match fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&dest)
            {
                Ok(o) => o,
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                    let meta = fs::symlink_metadata(&dest).map_err(|e| Error::io(&dest, e))?;
                    if meta.file_type().is_symlink() || !meta.is_file() {
                        return Err(Error::UnsafeOverride(rel.to_string()));
                    }
                    fs::File::create(&dest).map_err(|e| Error::io(&dest, e))?
                }
                Err(e) => return Err(Error::io(&dest, e)),
            };
            let max = limits
                .entry_size
                .min(limits.total_size.saturating_sub(total));
            let n =
                io::copy(&mut (&mut f).take(max + 1), &mut out).map_err(|e| Error::io(&dest, e))?;
            if n > max {
                return Err(mango_archive::ArchiveError::EntryTooLarge(name).into());
            }
            total += n;
        }
    }
    Ok(())
}

/// What a pack would install, read without touching an instance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackSummary {
    pub info: ModpackInfo,
    /// Sum of the sizes of the files that would be downloaded.
    pub download_bytes: u64,
}

/// Validate a downloaded `.mrpack` and summarize it. `scratch_root` is only
/// used to check override paths; it does not have to exist and nothing is
/// written there.
pub fn inspect_pack(pack: &Path, scratch_root: &Path) -> Result<PackSummary> {
    let index = read_pack(pack, scratch_root)?;
    let files: Vec<&PackFile> = index
        .files
        .iter()
        .filter(|f| f.env.as_ref().and_then(|e| e.client.as_deref()) != Some("unsupported"))
        .collect();
    let (loader, loader_version) = detect_loader(&index.dependencies);
    Ok(PackSummary {
        download_bytes: files.iter().filter_map(|f| f.file_size).sum(),
        info: ModpackInfo {
            name: index.name.clone(),
            mc_version: index.dependencies.get("minecraft").cloned(),
            loader,
            loader_version,
            file_count: files.len(),
        },
    })
}

/// Install a Modrinth modpack version into a profile's instance.
pub async fn install_modpack(
    client: &Client,
    paths: &Paths,
    profile: &Profile,
    version_id: &str,
    on_log: &LogFn,
    on_progress: Option<Arc<ProgressFn>>,
) -> Result<ModpackInfo> {
    let version = client.version(version_id).await?;
    let file = primary_file(&version).ok_or_else(|| invalid("version has no file"))?;
    let cache = paths.cache();
    let pack_path = safe_archive_path(&cache, safe_filename(&file.filename)?)?;
    on_log(&format!("Downloading modpack {}…", file.filename));
    let result = async {
        download_file(
            &file.url,
            &pack_path,
            &DownloadOptions {
                expected: expected_for(file),
                ..DownloadOptions::default()
            },
        )
        .await?;
        install_from_pack(&pack_path, paths, profile, on_progress).await
    }
    .await;
    let _ = tokio::fs::remove_file(&pack_path).await;
    result
}

async fn install_from_pack(
    pack_path: &Path,
    paths: &Paths,
    profile: &Profile,
    on_progress: Option<Arc<ProgressFn>>,
) -> Result<ModpackInfo> {
    let game_dir = paths.instance_dir(&profile.id)?;
    let (p, g) = (pack_path.to_path_buf(), game_dir.clone());
    let index = tokio::task::spawn_blocking(move || read_pack(&p, &g))
        .await
        .map_err(|e| invalid(&e.to_string()))??;

    let files: Vec<PackFile> = index
        .files
        .into_iter()
        .filter(|f| f.env.as_ref().and_then(|e| e.client.as_deref()) != Some("unsupported"))
        .collect();
    if files.len() > Limits::default().entries {
        return Err(invalid("too many downloads"));
    }

    struct Job {
        dest: PathBuf,
        url: String,
        expected: Expected,
        label: String,
    }
    let mut jobs = Vec::with_capacity(files.len());
    for f in &files {
        if f.path.is_empty() {
            return Err(invalid("file without a path"));
        }
        let dest = safe_archive_path(&game_dir, &f.path)?;
        let url = f
            .downloads
            .first()
            .cloned()
            .ok_or_else(|| Error::NoDownload(f.path.clone()))?;
        jobs.push(Job {
            label: dest
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
            dest,
            url,
            expected: Expected {
                sha1: f.hashes.sha1.clone(),
                sha512: f.hashes.sha512.clone(),
                size: f.file_size,
                ..Expected::default()
            },
        });
    }

    let total = jobs.len();
    let done = Arc::new(AtomicUsize::new(0));
    let results = pool(jobs, 8, move |job, _| {
        let (done, on_progress) = (done.clone(), on_progress.clone());
        async move {
            download_file(
                &job.url,
                &job.dest,
                &DownloadOptions {
                    expected: job.expected,
                    ..DownloadOptions::default()
                },
            )
            .await?;
            let n = done.fetch_add(1, Ordering::SeqCst) + 1;
            if let Some(cb) = &on_progress {
                cb(&ModpackProgress {
                    done: n,
                    total,
                    label: job.label,
                });
            }
            Ok::<(), Error>(())
        }
    })
    .await?;
    results.into_iter().collect::<Result<Vec<_>>>()?;

    let (p, g) = (pack_path.to_path_buf(), game_dir);
    tokio::task::spawn_blocking(move || extract_overrides(&p, &g))
        .await
        .map_err(|e| invalid(&e.to_string()))??;

    let (loader, loader_version) = detect_loader(&index.dependencies);
    Ok(ModpackInfo {
        name: index.name,
        mc_version: index.dependencies.get("minecraft").cloned(),
        loader,
        loader_version,
        file_count: total,
    })
}
