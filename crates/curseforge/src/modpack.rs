//! CurseForge modpack import (`manifest.json` + `overrides/`).

use std::collections::HashMap;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use mango_archive::{safe_archive_path, validate_entries, ArchiveError, EntryInfo, Limits};
use mango_core::Loader;
use mango_net::{download_file, pool, DownloadOptions, Expected};
use serde::Deserialize;

use crate::client::Client;
use crate::error::{Error, Result};
use crate::types::{website_url, ClassId, File, Project};

pub const MAX_MANIFEST_BYTES: u64 = 4 * 1024 * 1024;
const MANIFEST_NAME: &str = "manifest.json";
const CONCURRENCY: usize = 8;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawLoader {
    #[serde(default)]
    id: String,
    #[serde(default)]
    primary: bool,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawMinecraft {
    #[serde(default)]
    version: String,
    #[serde(default)]
    mod_loaders: Vec<RawLoader>,
}

#[derive(Debug, Deserialize)]
struct RawFile {
    #[serde(rename = "projectID")]
    project_id: u64,
    #[serde(rename = "fileID")]
    file_id: u64,
    #[serde(default = "yes")]
    required: bool,
}

fn yes() -> bool {
    true
}

#[derive(Debug, Default, Deserialize)]
struct RawManifest {
    #[serde(default)]
    minecraft: RawMinecraft,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    version: Option<String>,
    #[serde(default)]
    files: Vec<RawFile>,
    #[serde(default)]
    overrides: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ManifestFile {
    pub project_id: u64,
    pub file_id: u64,
    pub required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manifest {
    pub name: Option<String>,
    pub version: Option<String>,
    pub mc_version: String,
    pub loader: Loader,
    pub loader_version: String,
    pub files: Vec<ManifestFile>,
    /// Folder inside the zip whose content is copied over the instance.
    pub overrides: String,
}

fn invalid(msg: &str) -> Error {
    Error::InvalidModpack(msg.into())
}

/// `fabric-0.16.9`, `neoforge-21.1.1`, `forge-47.2.0`, `quilt-0.26.0`.
pub fn parse_loader_id(id: &str) -> Result<(Loader, String)> {
    let (name, version) = id
        .split_once('-')
        .ok_or_else(|| invalid("unreadable loader id"))?;
    let loader = match name.to_ascii_lowercase().as_str() {
        "fabric" => Loader::Fabric,
        "quilt" => Loader::Quilt,
        "forge" => Loader::Forge,
        "neoforge" => Loader::Neoforge,
        _ => return Err(invalid("unsupported mod loader")),
    };
    if version.is_empty() {
        return Err(invalid("loader id without a version"));
    }
    Ok((loader, version.to_string()))
}

/// An `overrides` value must be one plain folder name.
fn valid_overrides_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name != "."
        && name != ".."
        && !name.contains(['/', '\\', '\0', ':'])
}

pub fn parse_manifest(bytes: &[u8]) -> Result<Manifest> {
    let raw: RawManifest =
        serde_json::from_slice(bytes).map_err(|e| Error::InvalidModpack(e.to_string()))?;
    if raw.minecraft.version.is_empty() {
        return Err(invalid("manifest has no Minecraft version"));
    }
    let primary = raw
        .minecraft
        .mod_loaders
        .iter()
        .find(|l| l.primary)
        .or(raw.minecraft.mod_loaders.first());
    let (loader, loader_version) = match primary {
        Some(l) => parse_loader_id(&l.id)?,
        None => (Loader::Vanilla, String::new()),
    };
    let overrides = raw.overrides.unwrap_or_else(|| "overrides".into());
    if !valid_overrides_name(&overrides) {
        return Err(invalid("unsafe overrides folder"));
    }
    if raw.files.len() > Limits::default().entries {
        return Err(invalid("too many files"));
    }
    Ok(Manifest {
        name: raw.name.filter(|n| !n.trim().is_empty()),
        version: raw.version,
        mc_version: raw.minecraft.version,
        loader,
        loader_version,
        files: raw
            .files
            .into_iter()
            .map(|f| ManifestFile {
                project_id: f.project_id,
                file_id: f.file_id,
                required: f.required,
            })
            .collect(),
        overrides,
    })
}

fn zip_err(e: zip::result::ZipError) -> Error {
    Error::InvalidModpack(e.to_string())
}

/// Read and parse `manifest.json` of a pack zip (blocking).
pub fn read_manifest(pack: &Path) -> Result<Manifest> {
    let file = fs::File::open(pack).map_err(|e| Error::io(pack, e))?;
    let mut zip = zip::ZipArchive::new(file).map_err(zip_err)?;
    if zip.len() > Limits::default().entries {
        return Err(invalid("too many entries"));
    }
    let mut entry = zip
        .by_name(MANIFEST_NAME)
        .map_err(|_| invalid("manifest.json is missing"))?;
    if entry.size() > MAX_MANIFEST_BYTES {
        return Err(invalid("manifest is too large"));
    }
    let mut buf = Vec::new();
    (&mut entry)
        .take(MAX_MANIFEST_BYTES + 1)
        .read_to_end(&mut buf)
        .map_err(|e| Error::io(pack, e))?;
    if buf.len() as u64 > MAX_MANIFEST_BYTES {
        return Err(invalid("manifest is too large"));
    }
    parse_manifest(&buf)
}

/// A pack file the user has to fetch from the website.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockedFile {
    pub name: String,
    pub url: String,
}

/// One resolved manifest entry.
#[derive(Debug, Clone)]
pub struct PackEntry {
    pub project_id: u64,
    pub file_id: u64,
    pub file: File,
    pub project: Option<Project>,
    /// Instance sub-folder (`mods`, `resourcepacks`, `shaderpacks`, ...).
    pub folder: &'static str,
    /// `None` = distribution blocked.
    pub url: Option<String>,
}

impl PackEntry {
    pub fn blocked(&self) -> Option<BlockedFile> {
        if self.url.is_some() {
            return None;
        }
        let class = self
            .project
            .as_ref()
            .and_then(Project::class)
            .unwrap_or(ClassId::Mods);
        let slug = self.project.as_ref().map_or("", |p| p.slug.as_str());
        let url = website_url(class, slug).unwrap_or_else(|| {
            format!(
                "https://www.curseforge.com/minecraft/{}",
                class.site_segment()
            )
        });
        let name = if self.file.file_name.is_empty() {
            self.file.display_name.clone()
        } else {
            self.file.file_name.clone()
        };
        Some(BlockedFile { name, url })
    }
}

#[derive(Debug, Clone)]
pub struct ResolvedPack {
    pub manifest: Manifest,
    pub entries: Vec<PackEntry>,
    /// Manifest entries marked `required: false`, not installed.
    pub skipped_optional: usize,
    /// Manifest entries the API does not know any more.
    pub missing: Vec<ManifestFile>,
}

impl ResolvedPack {
    pub fn downloadable(&self) -> impl Iterator<Item = &PackEntry> {
        self.entries.iter().filter(|e| e.url.is_some())
    }

    pub fn blocked(&self) -> Vec<BlockedFile> {
        self.entries.iter().filter_map(PackEntry::blocked).collect()
    }

    pub fn download_bytes(&self) -> u64 {
        self.downloadable().map(|e| e.file.file_length).sum()
    }
}

fn folder_for(class: Option<ClassId>) -> &'static str {
    match class {
        Some(ClassId::ResourcePacks) => "resourcepacks",
        Some(ClassId::Shaders) => "shaderpacks",
        Some(ClassId::DataPacks) => "datapacks",
        _ => "mods",
    }
}

/// Look up every required manifest file (two batch calls) and decide which
/// ones may be downloaded. Blocked files are reported, never an error.
pub async fn resolve_pack(client: &Client, manifest: &Manifest) -> Result<ResolvedPack> {
    let wanted: Vec<ManifestFile> = manifest
        .files
        .iter()
        .filter(|f| f.required)
        .copied()
        .collect();
    let skipped_optional = manifest.files.len() - wanted.len();

    let file_ids: Vec<u64> = wanted.iter().map(|f| f.file_id).collect();
    let mut project_ids: Vec<u64> = wanted.iter().map(|f| f.project_id).collect();
    project_ids.sort_unstable();
    project_ids.dedup();
    let files: HashMap<u64, File> = client
        .files_by_ids(&file_ids)
        .await?
        .into_iter()
        .map(|f| (f.id, f))
        .collect();
    let projects: HashMap<u64, Project> = client
        .projects(&project_ids)
        .await?
        .into_iter()
        .map(|p| (p.id, p))
        .collect();

    let mut entries = Vec::new();
    let mut missing = Vec::new();
    for m in wanted {
        let Some(file) = files.get(&m.file_id).cloned() else {
            missing.push(m);
            continue;
        };
        entries.push(PackEntry {
            project_id: m.project_id,
            file_id: m.file_id,
            folder: folder_for(projects.get(&m.project_id).and_then(Project::class)),
            project: projects.get(&m.project_id).cloned(),
            file,
            url: None,
        });
    }

    // Files with a url in the batch answer need no further call; the rest ask
    // the download endpoint (which refuses when the author opted out).
    let client2 = client.clone();
    let urls = pool(
        entries
            .iter()
            .map(|e| (e.project.clone(), e.file.clone()))
            .collect::<Vec<_>>(),
        CONCURRENCY,
        move |(project, file), _| {
            let client = client2.clone();
            async move { client.resolve_download_url(project.as_ref(), &file).await }
        },
    )
    .await?;
    for (entry, url) in entries.iter_mut().zip(urls) {
        // A failing lookup must not sink the whole pack: treat as blocked.
        entry.url = url.unwrap_or(None);
    }
    Ok(ResolvedPack {
        manifest: manifest.clone(),
        entries,
        skipped_optional,
        missing,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackProgress {
    pub done: usize,
    pub total: usize,
    pub label: String,
}

pub type ProgressFn = dyn Fn(&PackProgress) + Send + Sync;

/// Filenames from the API must be a bare basename.
pub fn safe_filename(name: &str) -> Result<&str> {
    let bare = !name.is_empty()
        && !name.contains(['/', '\\', '\0'])
        && name != "."
        && name != ".."
        && Path::new(name).file_name().and_then(|n| n.to_str()) == Some(name);
    if bare {
        Ok(name)
    } else {
        Err(Error::InvalidFilename)
    }
}

/// Validate every override entry (count, sizes, symlinks, paths) before any
/// download starts.
fn validate_overrides(pack: &Path, game_dir: &Path, folder: &str) -> Result<()> {
    let limits = Limits::default();
    let file = fs::File::open(pack).map_err(|e| Error::io(pack, e))?;
    let mut zip = zip::ZipArchive::new(file).map_err(zip_err)?;
    if zip.len() > limits.entries {
        return Err(invalid("too many entries"));
    }
    let prefix = format!("{folder}/");
    let mut entries = Vec::new();
    for i in 0..zip.len() {
        let f = zip.by_index_raw(i).map_err(zip_err)?;
        if f.name().starts_with(&prefix) {
            entries.push(EntryInfo {
                name: f.name().to_string(),
                size: f.size(),
                is_dir: f.is_dir(),
                unix_mode: f.unix_mode(),
            });
        }
    }
    validate_entries(entries.iter().cloned(), &limits)?;
    for e in entries.iter().filter(|e| !e.is_dir) {
        safe_archive_path(game_dir, &e.name[prefix.len()..])?;
    }
    Ok(())
}

/// Copy `<folder>/` of the pack into the game dir. New files are created
/// exclusively; an existing target is overwritten only when it is a regular
/// file (never a symlink).
fn extract_overrides(pack: &Path, game_dir: &Path, folder: &str) -> Result<()> {
    let limits = Limits::default();
    let file = fs::File::open(pack).map_err(|e| Error::io(pack, e))?;
    let mut zip = zip::ZipArchive::new(file).map_err(zip_err)?;
    let prefix = format!("{folder}/");
    let mut total = 0u64;
    for i in 0..zip.len() {
        let mut f = zip.by_index(i).map_err(zip_err)?;
        let name = f.name().to_string();
        if !name.starts_with(&prefix) || f.is_dir() {
            continue;
        }
        let rel = &name[prefix.len()..];
        // The launcher's own bookkeeping is never taken from a pack.
        if rel.is_empty() || rel.starts_with(".mangoclient-") {
            continue;
        }
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
        let n = io::copy(&mut (&mut f).take(max + 1), &mut out).map_err(|e| Error::io(&dest, e))?;
        if n > max {
            return Err(ArchiveError::EntryTooLarge(name).into());
        }
        total += n;
    }
    Ok(())
}

/// Download every non-blocked file (8 at a time, SHA-1 and size checked) and
/// then copy the overrides over them. The API key is not sent to the CDN.
pub async fn install_resolved(
    pack: &ResolvedPack,
    pack_zip: &Path,
    game_dir: &Path,
    on_progress: Option<Arc<ProgressFn>>,
) -> Result<()> {
    let folder = pack.manifest.overrides.clone();
    let (p, g, f) = (
        pack_zip.to_path_buf(),
        game_dir.to_path_buf(),
        folder.clone(),
    );
    tokio::task::spawn_blocking(move || validate_overrides(&p, &g, &f))
        .await
        .map_err(|e| invalid(&e.to_string()))??;

    struct Job {
        dest: PathBuf,
        url: String,
        expected: Expected,
        label: String,
    }
    let mut jobs = Vec::new();
    for e in pack.downloadable() {
        let name = safe_filename(&e.file.file_name)?;
        let dest = safe_archive_path(game_dir, &format!("{}/{name}", e.folder))?;
        jobs.push(Job {
            dest,
            url: e.url.clone().unwrap_or_default(),
            expected: Expected {
                sha1: e.file.sha1(),
                size: (e.file.file_length > 0).then_some(e.file.file_length),
                ..Expected::default()
            },
            label: name.to_string(),
        });
    }
    let total = jobs.len();
    let done = Arc::new(AtomicUsize::new(0));
    let results = pool(jobs, CONCURRENCY, move |job, _| {
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
                cb(&PackProgress {
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

    let (p, g) = (pack_zip.to_path_buf(), game_dir.to_path_buf());
    tokio::task::spawn_blocking(move || extract_overrides(&p, &g, &folder))
        .await
        .map_err(|e| invalid(&e.to_string()))??;
    Ok(())
}
