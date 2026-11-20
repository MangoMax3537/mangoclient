//! Part 5 of `docs/ipc.md`: blocked CurseForge pack files.
//!
//! A pack file whose author forbids third-party downloads is (1) looked up on
//! Modrinth by SHA-1 (accepted only when that build lists exactly one loader)
//! and otherwise (2) fetched by the user from the CurseForge page, which a
//! per-profile watcher notices in the download folder, verifies and copies
//! into the instance. The list lives in a sidecar next to the instance so it
//! survives restarts.

use crate::error::{AppError, Res};
use crate::events::{BlockedChangedEvent, ContentChangedEvent, Event};
use crate::install::safe_name;
use crate::models::{BlockedFile, BlockedModrinth, BlockedState, ContentType};
use crate::service::{lock, now_ms, AppService, Inner};
use crate::types::ProfileView;
use mango_archive::safe_archive_path;
use mango_core::persistence::{read_json_value, write_json_atomic};
use mango_core::{ModRecord, Paths, Profile};
use mango_curseforge as cf;
use mango_modrinth as modrinth;
use mango_net::{download_file, hash_file, DownloadOptions, Expected, HashAlgo};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Weak};
use std::time::{Duration, SystemTime};

/// Next to the instance's `mods/`: the list of blocked files of its pack.
pub(crate) const SIDECAR: &str = ".mangoclient-blocked.json";

/// Browsers write these while a download is running.
const TEMP_SUFFIXES: [&str; 5] = [".crdownload", ".part", ".tmp", ".download", ".opdownload"];
/// Bounds of one directory scan.
const MAX_SCAN_ENTRIES: usize = 50_000;
const MAX_DEPTH: usize = 8;
/// Larger files are never hashed when the expected size is unknown.
const MAX_HASH_BYTES: u64 = 1 << 30;

// ---------------------------------------------------------------- persistence

#[derive(Serialize)]
struct SidecarOut<'a> {
    version: u32,
    files: &'a [BlockedFile],
}

fn sidecar_path(paths: &Paths, profile_id: &str) -> Option<PathBuf> {
    paths.instance_dir(profile_id).ok().map(|d| d.join(SIDECAR))
}

/// The stored list; unreadable entries are dropped, never fatal.
pub(crate) fn load(paths: &Paths, profile_id: &str) -> Vec<BlockedFile> {
    let Some(file) = sidecar_path(paths, profile_id) else {
        return Vec::new();
    };
    let Some(raw) = read_json_value(&file) else {
        return Vec::new();
    };
    raw.get("files")
        .and_then(|f| f.as_array())
        .map(|list| {
            list.iter()
                .filter_map(|v| serde_json::from_value::<BlockedFile>(v.clone()).ok())
                .filter(|f| safe_name(&f.file_name).is_ok())
                .collect()
        })
        .unwrap_or_default()
}

/// Write the list atomically; an empty list removes the sidecar.
pub(crate) fn save(paths: &Paths, profile_id: &str, files: &[BlockedFile]) -> Res<()> {
    let file = sidecar_path(paths, profile_id).ok_or_else(AppError::profile_not_found)?;
    if files.is_empty() {
        let _ = fs::remove_file(&file);
        let _ = fs::remove_file(mango_core::persistence::backup_path(&file));
        return Ok(());
    }
    write_json_atomic(&file, &SidecarOut { version: 1, files })?;
    Ok(())
}

fn waiting(f: &BlockedFile) -> bool {
    matches!(f.state, BlockedState::Pending | BlockedState::Found)
}

/// Pending plus found files.
pub(crate) fn pending_count(paths: &Paths, profile_id: &str) -> usize {
    load(paths, profile_id)
        .iter()
        .filter(|f| waiting(f))
        .count()
}

/// The UI's profile with `pendingBlocked` filled in.
pub(crate) fn view_of(paths: &Paths, p: &Profile, default_ram_mb: u32) -> ProfileView {
    let mut view = ProfileView::new(p, default_ram_mb);
    view.pending_blocked = pending_count(paths, &p.id);
    view.auto_block = crate::block_icons::auto_block(paths, &p.id);
    // A recorded picture whose file is gone means automatic.
    if matches!(
        view.background,
        crate::types::ProfileBackground::Image { .. }
    ) && !paths.background_file(&p.id).is_ok_and(|f| f.is_file())
    {
        view.background = crate::types::ProfileBackground::Auto {
            dim: p.background_dim,
            blur: p.background_blur,
        };
    }
    view
}

/// `<instance>/<content folder>/<name>`, resolved through the safe-path
/// helper from the instance root so a linked content folder is refused too.
fn instance_target(paths: &Paths, profile_id: &str, kind: ContentType, name: &str) -> Res<PathBuf> {
    let instance = paths.instance_dir(profile_id)?;
    let folder = modrinth::target_dir_for(paths, kind, profile_id)?;
    let rel = folder
        .strip_prefix(&instance)
        .map_err(|_| AppError::new("unsafe_path", "content folder outside the instance"))?;
    let rel = format!(
        "{}/{}",
        rel.to_string_lossy().replace('\\', "/"),
        safe_name(name)?
    );
    safe_archive_path(&instance, &rel).map_err(|e| AppError::new("unsafe_path", e.to_string()))
}

// ---------------------------------------------------------------- Modrinth fallback

/// A blocked file Modrinth also hosts.
pub(crate) struct ModrinthMatch {
    pub project: modrinth::Project,
    pub version: modrinth::Version,
    pub file: modrinth::VersionFile,
}

pub(crate) struct BlockedEntry {
    pub file: BlockedFile,
    pub matched: Option<ModrinthMatch>,
}

fn content_type(folder: &str) -> ContentType {
    match folder {
        "resourcepacks" => ContentType::Resourcepack,
        "shaderpacks" => ContentType::Shader,
        "datapacks" => ContentType::Datapack,
        _ => ContentType::Mod,
    }
}

impl AppService {
    /// The blocked files of a resolved pack, each looked up on Modrinth when
    /// the config allows it. A failing lookup only leaves files pending.
    pub(crate) async fn blocked_entries(&self, pack: &cf::ResolvedPack) -> Vec<BlockedEntry> {
        let mut entries: Vec<BlockedEntry> = pack
            .entries
            .iter()
            .filter(|e| e.url.is_none())
            .filter_map(|e| {
                let b = e.blocked()?;
                let file_name = e.file.file_name.clone();
                if safe_name(&file_name).is_err() {
                    tracing::warn!("blocked pack file with an unusable name: {file_name:?}");
                    return None;
                }
                let name = e
                    .project
                    .as_ref()
                    .map(|p| p.name.clone())
                    .filter(|n| !n.is_empty())
                    .unwrap_or_else(|| b.name.clone());
                Some(BlockedEntry {
                    file: BlockedFile {
                        name,
                        file_name,
                        url: b.url,
                        sha1: e.file.sha1(),
                        size: (e.file.file_length > 0).then_some(e.file.file_length),
                        kind: content_type(e.folder),
                        state: BlockedState::Pending,
                        modrinth: None,
                        found_path: None,
                    },
                    matched: None,
                })
            })
            .collect();
        let fallback = self
            .inner
            .with_store(|s| s.config.blocked_mods_modrinth_fallback);
        if fallback && !entries.is_empty() {
            self.match_on_modrinth(&mut entries).await;
        }
        entries
    }

    async fn match_on_modrinth(&self, entries: &mut [BlockedEntry]) {
        let hashes: Vec<String> = entries.iter().filter_map(|e| e.file.sha1.clone()).collect();
        if hashes.is_empty() {
            return;
        }
        let client = &self.inner.modrinth;
        let found = match client.versions_by_sha1(&hashes).await {
            Ok(f) => f,
            Err(e) => {
                tracing::warn!("Modrinth hash lookup failed: {e}");
                return;
            }
        };
        let mut picked: Vec<(usize, modrinth::Version, modrinth::VersionFile)> = Vec::new();
        for (i, entry) in entries.iter().enumerate() {
            let Some(sha1) = entry.file.sha1.as_deref() else {
                continue;
            };
            let Some(version) = found.get(sha1) else {
                continue;
            };
            // Ambiguous builds (several loaders) are the user's call.
            if version.loaders.len() != 1 {
                continue;
            }
            let Some(file) = version.files.iter().find(|f| {
                f.hashes
                    .sha1
                    .as_deref()
                    .is_some_and(|h| h.eq_ignore_ascii_case(sha1))
            }) else {
                continue;
            };
            if safe_name(&file.filename).is_err() || file.url.is_empty() {
                continue;
            }
            picked.push((i, version.clone(), file.clone()));
        }
        if picked.is_empty() {
            return;
        }
        let mut ids: Vec<String> = picked
            .iter()
            .map(|(_, v, _)| v.project_id.clone())
            .collect();
        ids.sort();
        ids.dedup();
        let projects: HashMap<String, modrinth::Project> = client
            .projects(&ids)
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|p| (p.id.clone(), p))
            .collect();
        for (i, version, file) in picked {
            let entry = &mut entries[i];
            let project = projects
                .get(&version.project_id)
                .cloned()
                .unwrap_or_else(|| modrinth::Project {
                    id: version.project_id.clone(),
                    slug: String::new(),
                    title: entry.file.name.clone(),
                    project_type: "mod".into(),
                    icon_url: None,
                    extra: Default::default(),
                });
            entry.file.state = BlockedState::MatchedModrinth;
            entry.file.modrinth = Some(BlockedModrinth {
                project_id: project.id.clone(),
                version_id: version.id.clone(),
                title: project.title.clone(),
            });
            entry.matched = Some(ModrinthMatch {
                project,
                version,
                file,
            });
        }
    }

    /// Download every Modrinth match into the instance. A failed download
    /// puts the file back to pending. Returns the records to store.
    pub(crate) async fn install_modrinth_matches(
        &self,
        profile_id: &str,
        entries: &mut [BlockedEntry],
        on_progress: &(dyn Fn(u64, u64, &str) + Send + Sync),
    ) -> Vec<ModRecord> {
        let total = entries.iter().filter(|e| e.matched.is_some()).count() as u64;
        let mut records = Vec::new();
        for entry in entries.iter_mut() {
            let Some(m) = entry.matched.take() else {
                continue;
            };
            on_progress(records.len() as u64, total, &m.file.filename);
            match self.download_match(profile_id, entry.file.kind, &m).await {
                Ok(rec) => records.push(rec),
                Err(e) => {
                    self.warn(
                        profile_id,
                        format!(
                            "Could not get {} from Modrinth ({e}); get it at {}",
                            entry.file.file_name, entry.file.url
                        ),
                    );
                    entry.file.state = BlockedState::Pending;
                    entry.file.modrinth = None;
                }
            }
        }
        records
    }

    async fn download_match(
        &self,
        profile_id: &str,
        kind: ContentType,
        m: &ModrinthMatch,
    ) -> Res<ModRecord> {
        let name = safe_name(&m.file.filename)?;
        let dest = instance_target(&self.inner.paths, profile_id, kind, name)?;
        download_file(
            &m.file.url,
            &dest,
            &DownloadOptions {
                expected: Expected {
                    sha512: m.file.hashes.sha512.clone(),
                    sha1: m.file.hashes.sha1.clone(),
                    size: m.file.size,
                    ..Expected::default()
                },
                ..DownloadOptions::default()
            },
        )
        .await?;
        Ok(ModRecord {
            project_id: Some(m.project.id.clone()),
            slug: Some(m.project.slug.clone()),
            title: Some(m.project.title.clone()),
            kind: Some(kind),
            icon: m.project.icon_url.clone(),
            version_id: Some(m.version.id.clone()),
            version_number: Some(m.version.version_number.clone()),
            filename: Some(name.to_string()),
            file: Some(dest.to_string_lossy().into_owned()),
            game_versions: Some(m.version.game_versions.clone()),
            loaders: Some(m.version.loaders.clone()),
            installed_at: Some(now_ms()),
            dependency: Some(false),
            required_dependencies: Some(Vec::new()),
            provider: Some("modrinth".to_string()),
            ..ModRecord::default()
        })
    }

    /// Persist the list next to the instance and tell the UI.
    pub(crate) fn store_blocked(&self, profile_id: &str, files: &[BlockedFile]) -> Res<()> {
        {
            let _gate = lock(&self.inner.blocked_gate);
            save(&self.inner.paths, profile_id, files)?;
        }
        emit_changed(&self.inner, profile_id, files);
        Ok(())
    }
}

fn emit_changed(inner: &Inner, profile_id: &str, files: &[BlockedFile]) {
    inner.emit(Event::BlockedChanged(BlockedChangedEvent {
        profile_id: profile_id.to_string(),
        files: files.to_vec(),
    }));
}

// ---------------------------------------------------------------- commands

fn not_found() -> AppError {
    AppError::new(
        "blocked_file_not_found",
        "That file is not on the blocked list",
    )
}

impl AppService {
    /// Folders the watcher looks at: the configured ones, else the Downloads
    /// folder.
    pub(crate) fn blocked_watch_dirs(&self) -> Vec<PathBuf> {
        let configured: Vec<PathBuf> = self.inner.with_store(|s| {
            s.config
                .blocked_mods_watch_dirs
                .iter()
                .map(PathBuf::from)
                .collect()
        });
        if !configured.is_empty() {
            return configured;
        }
        self.inner
            .opts
            .downloads_dir
            .clone()
            .or_else(mango_core::default_downloads_dir)
            .into_iter()
            .collect()
    }

    pub fn blocked_files(&self, profile_id: String) -> Res<Vec<BlockedFile>> {
        self.profile(&profile_id)?;
        Ok(load(&self.inner.paths, &profile_id))
    }

    /// Start watching for this profile's pending files. Returns the watched
    /// folders. Nothing is spawned when there is nothing to wait for.
    pub async fn blocked_watch_start(&self, profile_id: String) -> Res<Vec<String>> {
        self.profile(&profile_id)?;
        let dirs = self.blocked_watch_dirs();
        let listed: Vec<String> = dirs.iter().map(|d| d.display().to_string()).collect();
        if !load(&self.inner.paths, &profile_id).iter().any(waiting) {
            return Ok(listed);
        }
        let mut watchers = lock(&self.inner.watchers);
        if watchers.get(&profile_id).is_some_and(|h| !h.is_finished()) {
            return Ok(listed);
        }
        let handle = self.spawn_watcher(profile_id.clone());
        watchers.insert(profile_id, handle);
        Ok(listed)
    }

    pub fn blocked_watch_stop(&self, profile_id: String) -> Res<()> {
        if let Some(h) = lock(&self.inner.watchers).remove(&profile_id) {
            h.abort();
        }
        Ok(())
    }

    /// Is a watcher running for this profile?
    pub fn blocked_watching(&self, profile_id: &str) -> bool {
        lock(&self.inner.watchers)
            .get(profile_id)
            .is_some_and(|h| !h.is_finished())
    }

    pub fn blocked_skip(&self, profile_id: String, file_name: String) -> Res<Vec<BlockedFile>> {
        self.profile(&profile_id)?;
        let files = {
            let _gate = lock(&self.inner.blocked_gate);
            let mut files = load(&self.inner.paths, &profile_id);
            let f = files
                .iter_mut()
                .find(|f| f.file_name == file_name)
                .ok_or_else(not_found)?;
            if !waiting(f) {
                return Ok(files);
            }
            f.state = BlockedState::Skipped;
            f.found_path = None;
            save(&self.inner.paths, &profile_id, &files)?;
            files
        };
        emit_changed(&self.inner, &profile_id, &files);
        if !files.iter().any(waiting) {
            self.blocked_watch_stop(profile_id)?;
        }
        Ok(files)
    }

    /// Opens the first watched folder and returns it.
    pub fn blocked_open_folder(&self) -> Res<String> {
        let dir = self
            .blocked_watch_dirs()
            .into_iter()
            .next()
            .ok_or_else(|| AppError::new("no_watch_dir", "No download folder is known"))?;
        if !dir.is_dir() {
            return Err(AppError::new(
                "watch_dir_missing",
                format!("{} does not exist", dir.display()),
            ));
        }
        self.open_dir(&dir);
        Ok(dir.display().to_string())
    }
}

// ---------------------------------------------------------------- watcher

/// One regular file seen in a watched folder.
#[derive(Debug, Clone)]
pub(crate) struct Seen {
    pub(crate) path: PathBuf,
    size: u64,
    modified: Option<SystemTime>,
}

/// A hash-less match must have been written after the watcher started, give
/// or take the coarse resolution of file timestamps.
const MTIME_SLACK: Duration = Duration::from_secs(2);

pub(crate) struct WatchState {
    /// When this watcher started; a file without a hash is only trusted when
    /// it is newer (a download the user just made).
    started: SystemTime,
    /// Size of every candidate at the previous poll.
    sizes: HashMap<PathBuf, u64>,
    /// SHA-1 of files already hashed, by identity (path, size, mtime).
    hashes: HashMap<(PathBuf, u64, Option<SystemTime>), String>,
    /// Copies that failed: never retried for the same file.
    failed: HashSet<(String, PathBuf, u64)>,
}

impl Default for WatchState {
    fn default() -> Self {
        Self {
            started: SystemTime::now(),
            sizes: HashMap::new(),
            hashes: HashMap::new(),
            failed: HashSet::new(),
        }
    }
}

pub(crate) enum Tick {
    Continue,
    Done,
}

pub(crate) fn is_temp_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    TEMP_SUFFIXES.iter().any(|s| lower.ends_with(s))
}

/// Regular files of the folders (symlinks are never followed), one level
/// deep or all the way down. Blocking.
pub(crate) fn scan(dirs: &[PathBuf], recursive: bool) -> Vec<Seen> {
    let mut out = Vec::new();
    let mut visited = HashSet::new();
    let mut count = 0usize;
    for root in dirs {
        let mut stack = vec![(root.clone(), 0usize)];
        while let Some((dir, depth)) = stack.pop() {
            let Ok(rd) = fs::read_dir(&dir) else {
                continue;
            };
            for entry in rd.flatten() {
                count += 1;
                if count > MAX_SCAN_ENTRIES {
                    return out;
                }
                let Ok(kind) = entry.file_type() else {
                    continue;
                };
                let path = entry.path();
                if kind.is_symlink() {
                    continue;
                }
                if kind.is_dir() {
                    if recursive && depth < MAX_DEPTH {
                        stack.push((path, depth + 1));
                    }
                    continue;
                }
                if !kind.is_file() || !visited.insert(path.clone()) {
                    continue;
                }
                if is_temp_name(&entry.file_name().to_string_lossy()) {
                    continue;
                }
                let Ok(meta) = entry.metadata() else {
                    continue;
                };
                out.push(Seen {
                    path,
                    size: meta.len(),
                    modified: meta.modified().ok(),
                });
            }
        }
    }
    out
}

impl AppService {
    fn spawn_watcher(&self, profile_id: String) -> tokio::task::JoinHandle<()> {
        let weak: Weak<Inner> = Arc::downgrade(&self.inner);
        let period = self.inner.opts.blocked_poll_interval;
        tokio::spawn(async move {
            let mut state = WatchState::default();
            let mut ticker = tokio::time::interval(period);
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            loop {
                ticker.tick().await;
                // Only alive while the service is.
                let Some(inner) = weak.upgrade() else {
                    return;
                };
                let svc = AppService { inner };
                if let Tick::Done = svc.watch_tick(&profile_id, &mut state).await {
                    // Another watcher may have replaced this entry already.
                    let mut w = lock(&svc.inner.watchers);
                    if w.get(&profile_id).is_some_and(|h| h.is_finished()) {
                        w.remove(&profile_id);
                    }
                    return;
                }
            }
        })
    }

    pub(crate) async fn watch_tick(&self, profile_id: &str, st: &mut WatchState) -> Tick {
        if self.profile(profile_id).is_err() {
            return Tick::Done;
        }
        let wanted: Vec<BlockedFile> = load(&self.inner.paths, profile_id)
            .into_iter()
            .filter(waiting)
            .collect();
        if wanted.is_empty() {
            return Tick::Done;
        }
        let dirs = self.blocked_watch_dirs();
        let recursive = self
            .inner
            .with_store(|s| s.config.blocked_mods_check_subfolders);
        let seen = tokio::task::spawn_blocking(move || scan(&dirs, recursive))
            .await
            .unwrap_or_default();
        // Complete = same size as one poll ago.
        let stable: Vec<Seen> = seen
            .iter()
            .filter(|s| st.sizes.get(&s.path) == Some(&s.size))
            .cloned()
            .collect();
        st.sizes = seen.iter().map(|s| (s.path.clone(), s.size)).collect();
        let live: HashSet<(PathBuf, u64, Option<SystemTime>)> = seen
            .iter()
            .map(|s| (s.path.clone(), s.size, s.modified))
            .collect();
        st.hashes.retain(|k, _| live.contains(k));

        for file in wanted {
            for candidate in &stable {
                if st.failed.contains(&(
                    file.file_name.clone(),
                    candidate.path.clone(),
                    candidate.size,
                )) {
                    continue;
                }
                if !self.candidate_matches(&file, candidate, st).await {
                    continue;
                }
                match self.accept_found(profile_id, &file, candidate).await {
                    Ok(()) => break,
                    Err(e) => {
                        tracing::warn!("could not take {}: {e}", file.file_name);
                        self.warn(
                            profile_id,
                            format!("Could not copy {} into the instance: {e}", file.file_name),
                        );
                        st.failed.insert((
                            file.file_name.clone(),
                            candidate.path.clone(),
                            candidate.size,
                        ));
                        // A file that failed its hash after copying must not
                        // be hashed again either.
                    }
                }
            }
        }
        let left = load(&self.inner.paths, profile_id).iter().any(waiting);
        if left {
            Tick::Continue
        } else {
            Tick::Done
        }
    }

    /// Does `candidate` look like `file`? Verifies the SHA-1 when the pack
    /// has one. Without a hash nothing can be proven, so the exact name and
    /// size must match AND the file must have been written after this watch
    /// started (an old file that merely looks alike is never installed).
    async fn candidate_matches(&self, file: &BlockedFile, c: &Seen, st: &mut WatchState) -> bool {
        let Some(want) = file.sha1.as_deref() else {
            let fresh = c
                .modified
                .is_some_and(|m| m >= st.started.checked_sub(MTIME_SLACK).unwrap_or(st.started));
            return fresh
                && file.size == Some(c.size)
                && c.path
                    .file_name()
                    .is_some_and(|n| n.to_string_lossy() == file.file_name);
        };
        match file.size {
            Some(size) if size != c.size => return false,
            None if c.size > MAX_HASH_BYTES => return false,
            _ => {}
        }
        let key = (c.path.clone(), c.size, c.modified);
        let got = match st.hashes.get(&key) {
            Some(h) => h.clone(),
            None => match hash_file(&c.path, HashAlgo::Sha1).await {
                Ok(h) => {
                    st.hashes.insert(key, h.clone());
                    h
                }
                Err(_) => return false,
            },
        };
        got.eq_ignore_ascii_case(want)
    }

    /// Copy a verified file into the instance and record the transition.
    async fn accept_found(&self, profile_id: &str, file: &BlockedFile, c: &Seen) -> Res<()> {
        self.set_blocked(profile_id, &file.file_name, |f| {
            f.state = BlockedState::Found;
            f.found_path = Some(c.path.display().to_string());
        })?;
        let name = safe_name(&file.file_name)?;
        let dest = instance_target(&self.inner.paths, profile_id, file.kind, name)?;
        let tmp = dest.with_file_name(format!(".{name}.mango-part"));
        let (src, tmp2) = (c.path.clone(), tmp.clone());
        tokio::task::spawn_blocking(move || copy_regular(&src, &tmp2)).await??;
        // The copy is what gets verified: the source may still change.
        let verified = match file.sha1.as_deref() {
            Some(want) => hash_file(&tmp, HashAlgo::Sha1)
                .await
                .is_ok_and(|h| h.eq_ignore_ascii_case(want)),
            None => fs::metadata(&tmp).is_ok_and(|m| Some(m.len()) == file.size),
        };
        if !verified {
            let _ = fs::remove_file(&tmp);
            self.set_blocked(profile_id, &file.file_name, |f| {
                f.state = BlockedState::Pending;
                f.found_path = None;
            })?;
            return Err(AppError::new(
                "blocked_hash_mismatch",
                "The file does not match the one the pack needs",
            ));
        }
        // Re-resolve: a link may have appeared since the first check.
        let dest = instance_target(&self.inner.paths, profile_id, file.kind, name)?;
        if let Err(e) = fs::rename(&tmp, &dest) {
            let _ = fs::remove_file(&tmp);
            return Err(e.into());
        }
        self.set_blocked(profile_id, &file.file_name, |f| {
            f.state = BlockedState::Installed;
        })?;
        self.register_copied(profile_id, file.kind).await;
        Ok(())
    }

    /// Update one entry of the list under the gate; persists and emits.
    fn set_blocked(
        &self,
        profile_id: &str,
        file_name: &str,
        change: impl FnOnce(&mut BlockedFile),
    ) -> Res<()> {
        let files = {
            let _gate = lock(&self.inner.blocked_gate);
            let mut files = load(&self.inner.paths, profile_id);
            let f = files
                .iter_mut()
                .find(|f| f.file_name == file_name)
                .ok_or_else(not_found)?;
            // A skip that raced the copy wins.
            if f.state == BlockedState::Skipped {
                return Err(AppError::new("blocked_skipped", "The file was skipped"));
            }
            change(f);
            save(&self.inner.paths, profile_id, &files)?;
            files
        };
        emit_changed(&self.inner, profile_id, &files);
        Ok(())
    }

    /// Let the mods folder scan pick the new jar up and tell the UI.
    async fn register_copied(&self, profile_id: &str, kind: ContentType) {
        if kind == ContentType::Mod {
            if let Ok(profile) = self.profile(profile_id) {
                let paths = self.inner.paths.clone();
                if let Ok(Ok(synced)) = tokio::task::spawn_blocking(move || {
                    mango_content::sync_profile_mods(&paths, &profile)
                })
                .await
                {
                    if let Err(e) = self.save_mods(profile_id, &synced.mods) {
                        tracing::warn!("could not record the copied file: {e}");
                    }
                }
            }
        }
        self.inner.emit(Event::ContentChanged(ContentChangedEvent {
            profile_id: profile_id.to_string(),
        }));
    }
}

/// Copy a regular file (never a link) to a new path. Blocking.
fn copy_regular(src: &Path, dest: &Path) -> io::Result<()> {
    let meta = fs::symlink_metadata(src)?;
    if !meta.file_type().is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "not a regular file",
        ));
    }
    if let Some(dir) = dest.parent() {
        fs::create_dir_all(dir)?;
    }
    let mut input = fs::File::open(src)?;
    // Never write through whatever sits at the temp path.
    match fs::remove_file(dest) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => return Err(e),
    }
    let mut out = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(dest)?;
    let result = io::copy(&mut input, &mut out).and_then(|_| out.sync_all());
    if result.is_err() {
        let _ = fs::remove_file(dest);
    }
    result
}
