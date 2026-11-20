//! Sodium, Lithium and FerriteCore for fabric/quilt instances.

use std::collections::{BTreeMap, HashSet};
use std::future::Future;
use std::path::{Path, PathBuf};

use mango_archive::safe_archive_path;
use mango_core::model::{Loader, Profile};
use mango_core::paths::Paths;
use mango_net::{download_file, DownloadOptions, Expected};
use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::error::ContentError;

const MANIFEST: &str = ".mangoclient-performance.json";
/// How long a Modrinth lookup is trusted before asking again.
const RESOLVE_TTL_MS: u64 = 7 * 24 * 60 * 60 * 1000;

/// (slug, display name, conflict regex)
pub const PERFORMANCE_MODS: [(&str, &str, &str); 3] = [
    (
        "sodium",
        "Sodium",
        r"(?i)sodium|embeddium|rubidium|optifine|vulkanmod|nvidium",
    ),
    ("lithium", "Lithium", r"(?i)lithium|canary"),
    ("ferrite-core", "FerriteCore", r"(?i)ferrite"),
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolvedFile {
    pub filename: String,
    pub url: String,
    #[serde(default)]
    pub sha512: Option<String>,
    #[serde(default)]
    pub sha1: Option<String>,
    #[serde(default)]
    pub size: Option<u64>,
}

/// Resolves what Modrinth offers; wired by the app. `Err` means offline or
/// unreachable (existing files are kept), `Ok(None)` means no build exists for
/// that Minecraft version.
pub trait ModrinthLookup: Send + Sync {
    fn resolve(
        &self,
        slug: &str,
        mc_version: &str,
    ) -> impl Future<Output = Result<Option<ResolvedFile>, ContentError>> + Send;

    /// Download a resolved file to `dest` (verified). Overridable for tests.
    fn download(
        &self,
        file: &ResolvedFile,
        dest: &Path,
    ) -> impl Future<Output = Result<(), ContentError>> + Send {
        let url = file.url.clone();
        let dest = dest.to_path_buf();
        let expected = Expected {
            sha1: file.sha1.clone(),
            sha256: None,
            sha512: file.sha512.clone(),
            size: file.size,
        };
        async move {
            let opts = DownloadOptions {
                expected,
                ..DownloadOptions::default()
            };
            download_file(&url, &dest, &opts)
                .await
                .map(|_| ())
                .map_err(|e| ContentError::Lookup(e.to_string()))
        }
    }
}

#[derive(Debug, Clone)]
pub struct PerfOptions<'a> {
    pub profile: &'a Profile,
    /// Global `config.performanceMods` (true unless switched off).
    pub global_enabled: bool,
    /// Per-profile `performanceMods` field; `Some(false)` opts out.
    pub profile_enabled: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PerfState {
    Off,
    Removed,
    Ok,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PerfOutcome {
    pub state: PerfState,
    pub installed: Vec<String>,
    pub skipped: Vec<String>,
    /// Log lines for the launch console.
    pub logs: Vec<String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct Manifest {
    #[serde(default)]
    files: BTreeMap<String, String>,
}

#[derive(Serialize, Deserialize)]
struct CacheEntry {
    at: u64,
    file: ResolvedFile,
}

fn is_plain_jar(name: &str) -> bool {
    Path::new(name).file_name().is_some_and(|n| n == name)
        && name.to_ascii_lowercase().ends_with(".jar")
}

fn read_manifest(mods_dir: &Path) -> Manifest {
    let Ok(text) = std::fs::read_to_string(mods_dir.join(MANIFEST)) else {
        return Manifest::default();
    };
    let Ok(mut m) = serde_json::from_str::<Manifest>(&text) else {
        return Manifest::default();
    };
    m.files.retain(|_, name| is_plain_jar(name));
    m
}

fn write_manifest(mods_dir: &Path, m: &Manifest) -> std::io::Result<()> {
    let text = serde_json::to_string_pretty(m).map_err(std::io::Error::other)?;
    std::fs::write(mods_dir.join(MANIFEST), text)
}

/// File names this module manages in `mods_dir` (for hiding from mod lists).
pub fn managed_names(mods_dir: &Path) -> HashSet<String> {
    read_manifest(mods_dir).files.into_values().collect()
}

/// True for a jar this module manages.
pub fn is_managed(mods_dir: &Path, filename: &str) -> bool {
    let base = Path::new(filename)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    managed_names(mods_dir).contains(&base)
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

fn cache_dir(paths: &Paths) -> PathBuf {
    paths.cache().join("performance-mods")
}

async fn resolve_cached<L: ModrinthLookup>(
    lookup: &L,
    paths: &Paths,
    slug: &str,
    mc: &str,
    now: u64,
) -> Result<Option<ResolvedFile>, ContentError> {
    let dir = cache_dir(paths);
    let cache_file = dir.join(format!("{slug}-{mc}.json"));
    if let Ok(text) = tokio::fs::read_to_string(&cache_file).await {
        if let Ok(c) = serde_json::from_str::<CacheEntry>(&text) {
            if now.saturating_sub(c.at) < RESOLVE_TTL_MS && now >= c.at {
                return Ok(Some(c.file));
            }
        }
    }
    let Some(file) = lookup.resolve(slug, mc).await? else {
        return Ok(None);
    };
    // A failed cache write only costs a lookup next launch.
    if tokio::fs::create_dir_all(&dir).await.is_ok() {
        if let Ok(text) = serde_json::to_string_pretty(&CacheEntry {
            at: now,
            file: file.clone(),
        }) {
            let _ = tokio::fs::write(&cache_file, text).await;
        }
    }
    Ok(Some(file))
}

fn remove_all(mods_dir: &Path, m: &Manifest) -> usize {
    let mut removed = 0;
    for name in m.files.values() {
        if std::fs::remove_file(mods_dir.join(name)).is_ok() {
            removed += 1;
        }
    }
    let _ = std::fs::remove_file(mods_dir.join(MANIFEST));
    removed
}

fn foreign_jars(mods_dir: &Path, m: &Manifest) -> Vec<String> {
    let ours: HashSet<&String> = m.files.values().collect();
    let Ok(rd) = std::fs::read_dir(mods_dir) else {
        return Vec::new();
    };
    let mut names: Vec<String> = rd
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| {
            let l = n.to_ascii_lowercase();
            (l.ends_with(".jar") || l.ends_with(".jar.disabled")) && !ours.contains(n)
        })
        .collect();
    names.sort();
    names
}

/// Put the performance mods into the instance, or take them out again.
/// Never fails and never blocks a launch: problems become `skipped` entries.
pub async fn ensure_performance_mods<L: ModrinthLookup>(
    paths: &Paths,
    opts: &PerfOptions<'_>,
    lookup: &L,
) -> PerfOutcome {
    ensure_at(paths, opts, lookup, now_ms()).await
}

pub(crate) async fn ensure_at<L: ModrinthLookup>(
    paths: &Paths,
    opts: &PerfOptions<'_>,
    lookup: &L,
    now: u64,
) -> PerfOutcome {
    let mut out = PerfOutcome {
        state: PerfState::Ok,
        installed: Vec::new(),
        skipped: Vec::new(),
        logs: Vec::new(),
    };
    let profile = opts.profile;
    let Ok(mods_dir) = crate::local::mods_dir(paths, &profile.id) else {
        out.state = PerfState::Off;
        return out;
    };
    let mut manifest = read_manifest(&mods_dir);
    let supported = matches!(profile.loader, Loader::Fabric | Loader::Quilt);
    let wanted = opts.profile_enabled != Some(false) && opts.global_enabled && supported;

    if !wanted {
        let removed = remove_all(&mods_dir, &manifest);
        if removed > 0 {
            out.logs
                .push("Performance mods removed from this profile".into());
        }
        out.state = if removed > 0 {
            PerfState::Removed
        } else {
            PerfState::Off
        };
        return out;
    }

    let foreign = foreign_jars(&mods_dir, &manifest);
    for (slug, name, conflict) in PERFORMANCE_MODS {
        let Ok(conflict) = Regex::new(conflict) else {
            continue;
        };
        let res = one_mod(
            paths,
            &mods_dir,
            &mut manifest,
            &foreign,
            (slug, &conflict),
            profile,
            lookup,
            now,
        )
        .await;
        match res {
            Step::Installed => out.installed.push(name.to_string()),
            Step::Skipped(why) => out.skipped.push(format!("{name} ({why})")),
            Step::Kept => {}
        }
    }

    if std::fs::create_dir_all(&mods_dir).is_ok() {
        // The copies still work without the manifest.
        let _ = write_manifest(&mods_dir, &manifest);
    }
    if !out.installed.is_empty() {
        out.logs.push(format!(
            "Performance mods: {} ready",
            out.installed.join(", ")
        ));
    }
    if !out.skipped.is_empty() {
        out.logs.push(format!(
            "Performance mods: skipped {}",
            out.skipped.join(", ")
        ));
    }
    out
}

enum Step {
    Installed,
    Skipped(String),
    Kept,
}

#[allow(clippy::too_many_arguments)]
async fn one_mod<L: ModrinthLookup>(
    paths: &Paths,
    mods_dir: &Path,
    manifest: &mut Manifest,
    foreign: &[String],
    (slug, conflict): (&str, &Regex),
    profile: &Profile,
    lookup: &L,
    now: u64,
) -> Step {
    if let Some(clash) = foreign.iter().find(|n| conflict.is_match(n)) {
        // The player brought their own; drop ours if we had one in.
        if let Some(own) = manifest.files.remove(slug) {
            let _ = std::fs::remove_file(mods_dir.join(own));
        }
        return Step::Skipped(format!("you have {clash}"));
    }
    let target = manifest.files.get(slug).map(|n| mods_dir.join(n));
    let file = match resolve_cached(lookup, paths, slug, &profile.mc_version, now).await {
        Ok(f) => f,
        Err(_) => {
            // Offline or Modrinth down: what is already in place stays.
            if target.as_ref().is_some_and(|t| t.exists()) {
                return Step::Kept;
            }
            return Step::Skipped("offline".into());
        }
    };
    let Some(file) = file else {
        // No build for this Minecraft version; take an old one out too.
        if let Some(t) = target {
            let _ = std::fs::remove_file(t);
        }
        manifest.files.remove(slug);
        return Step::Skipped(format!("no build for {}", profile.mc_version));
    };
    if !is_plain_jar(&file.filename) {
        return Step::Skipped("invalid filename".into());
    }
    let dest = match safe_archive_path(mods_dir, &file.filename) {
        Ok(d) => d,
        Err(e) => return Step::Skipped(e.to_string()),
    };
    if manifest.files.get(slug) == Some(&file.filename) && dest.exists() {
        return Step::Kept;
    }
    let cache = cache_dir(paths);
    let cached = match safe_archive_path(&cache, &file.filename) {
        Ok(c) => c,
        Err(e) => return Step::Skipped(e.to_string()),
    };
    if !cached.exists() {
        if let Err(e) = tokio::fs::create_dir_all(&cache).await {
            return Step::Skipped(e.to_string());
        }
        if let Err(e) = lookup.download(&file, &cached).await {
            return Step::Skipped(e.to_string());
        }
    }
    if let Err(e) = tokio::fs::create_dir_all(mods_dir).await {
        return Step::Skipped(e.to_string());
    }
    // An older build has to go first: two jars, one mod id, no launch.
    if let Some(t) = &target {
        if manifest.files.get(slug) != Some(&file.filename) {
            let _ = std::fs::remove_file(t);
        }
    }
    if let Err(e) = tokio::fs::copy(&cached, &dest).await {
        return Step::Skipped(e.to_string());
    }
    manifest.files.insert(slug.to_string(), file.filename);
    Step::Installed
}
