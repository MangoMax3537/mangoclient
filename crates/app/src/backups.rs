//! Part 9 of `docs/ipc.md`: world backups and instance snapshots.
//!
//! Everything lives under `<root>/backups/<profileId>/`, never inside the
//! instance. One backup is two files named after its id:
//!
//! * `<createdAtMs>-<kind>-<safeLabel>.zip`, the data, and
//! * `<id>.json`, the [`Backup`] record (a backup without both is ignored).
//!
//! A world backup holds one top-level folder named like the world. A snapshot
//! holds `mods/`, `config/`, `resourcepacks/`, `shaderpacks/` (those that
//! existed) plus `mangoclient-profile.json` with the profile's mod records.
//! Restoring never overwrites a world and always extracts through the safe
//! extraction of `mango-archive` into a staging folder first.

use crate::error::{AppError, Res};
use crate::events::{ContentChangedEvent, Event};
use crate::service::{lock, now_ms, AppService, Inner};
use crate::worlds::{safe_folder_name, world_path};
use mango_archive::{extract_zip, ArchiveError, Limits};
use mango_core::{ModRecord, Paths, Profile};
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::{self, Read, Seek, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use ts_rs::TS;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

/// Bounds for creating and restoring a backup. Worlds are far bigger than
/// the mod archives the default limits are meant for.
pub(crate) const LIMITS: Limits = Limits {
    entries: 500_000,
    entry_size: 8 * 1024 * 1024 * 1024,
    total_size: 64 * 1024 * 1024 * 1024,
};

/// The profile record inside a snapshot.
pub(crate) const MANIFEST: &str = "mangoclient-profile.json";
/// The instance folders a snapshot covers.
pub(crate) const SNAPSHOT_DIRS: [&str; 4] = ["mods", "config", "resourcepacks", "shaderpacks"];

const MAX_MANIFEST: u64 = 32 * 1024 * 1024;
const MAX_SIDECAR: u64 = 64 * 1024;
const MAX_LABEL_CHARS: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
pub enum BackupKind {
    World,
    Snapshot,
}

impl BackupKind {
    fn as_str(self) -> &'static str {
        match self {
            BackupKind::World => "world",
            BackupKind::Snapshot => "snapshot",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum BackupReason {
    Manual,
    BeforeUpdate,
    BeforeDelete,
    Auto,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Backup {
    /// The file stem, `<createdAtMs>-<kind>-<safeLabel>`.
    pub id: String,
    pub kind: BackupKind,
    pub profile_id: String,
    /// The world's folder name; `null` for snapshots.
    pub world_id: Option<String>,
    /// Epoch ms.
    pub created_at: u64,
    pub size_bytes: u64,
    pub label: String,
    pub reason: BackupReason,
}

/// `mangoclient-profile.json` inside a snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SnapshotManifest {
    pub version: u32,
    pub profile_id: String,
    pub name: String,
    pub mc_version: String,
    pub loader: mango_core::Loader,
    #[serde(default)]
    pub loader_version: String,
    /// Where the instance lived when the snapshot was taken (to re-base the
    /// absolute `file` paths of the mod records).
    #[serde(default)]
    pub instance_dir: String,
    /// The folders that existed then.
    #[serde(default)]
    pub folders: Vec<String>,
    #[serde(default)]
    pub mods: Vec<ModRecord>,
    #[serde(default)]
    pub created_at: u64,
}

// ---- small helpers ----

fn game_running() -> AppError {
    AppError::new(
        "game_running",
        "Close the game before changing this instance",
    )
}

fn not_found() -> AppError {
    AppError::new("backup_not_found", "Backup not found")
}

fn corrupt(detail: impl std::fmt::Display) -> AppError {
    AppError::new("backup_corrupt", format!("The backup is damaged: {detail}"))
}

fn failed(detail: impl std::fmt::Display) -> AppError {
    AppError::new("backup_failed", format!("Backup failed: {detail}"))
}

fn archive_error(e: ArchiveError) -> AppError {
    match e {
        ArchiveError::Io(e) => AppError::from(e),
        other => corrupt(other),
    }
}

fn is_real_dir(p: &Path) -> bool {
    fs::symlink_metadata(p).is_ok_and(|m| m.is_dir())
}

/// A zip entry name that stays inside its archive root: relative, no `..`,
/// no empty or `.` parts, no backslashes or NUL.
pub(crate) fn zip_name_ok(name: &str) -> bool {
    if name.is_empty() || name.contains(['\0', '\\']) || name.starts_with('/') {
        return false;
    }
    let trimmed = name.strip_suffix('/').unwrap_or(name);
    !trimmed.is_empty()
        && trimmed
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..")
}

/// File-name part of a label: `[A-Za-z0-9_-]`, at most 40 characters.
fn file_label(label: &str) -> String {
    let mut out = String::new();
    for c in label.chars() {
        let c = if c.is_ascii_alphanumeric() || c == '_' {
            c
        } else {
            '-'
        };
        if c == '-' && out.ends_with('-') {
            continue;
        }
        out.push(c);
    }
    let out: String = out.trim_matches('-').chars().take(40).collect();
    let out = out.trim_matches('-').to_string();
    if out.is_empty() {
        "backup".to_string()
    } else {
        out
    }
}

/// The label users see: no control characters, trimmed, bounded.
fn display_label(label: &str, fallback: &str) -> String {
    let cleaned: String = label.chars().filter(|c| !c.is_control()).collect();
    let cleaned: String = cleaned.trim().chars().take(MAX_LABEL_CHARS).collect();
    if cleaned.is_empty() {
        fallback.to_string()
    } else {
        cleaned
    }
}

/// `<digits>-<world|snapshot>-<label>`: what [`write_backup`] produces.
fn valid_backup_id(id: &str) -> Option<BackupKind> {
    if id.is_empty()
        || id.len() > 160
        || !id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        return None;
    }
    let (ms, rest) = id.split_once('-')?;
    if ms.is_empty() || !ms.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    if rest.starts_with("world-") {
        Some(BackupKind::World)
    } else if rest.starts_with("snapshot-") {
        Some(BackupKind::Snapshot)
    } else {
        None
    }
}

fn zip_path(dir: &Path, id: &str) -> PathBuf {
    dir.join(format!("{id}.zip"))
}

fn sidecar_path(dir: &Path, id: &str) -> PathBuf {
    dir.join(format!("{id}.json"))
}

// ---- writing ----

struct Budget {
    entries: usize,
    total: u64,
}

impl Budget {
    fn entry(&mut self) -> Res<()> {
        self.entries += 1;
        if self.entries > LIMITS.entries {
            return Err(too_large());
        }
        Ok(())
    }

    fn bytes(&mut self, n: u64) -> Res<()> {
        if n > LIMITS.entry_size {
            return Err(too_large());
        }
        self.total = self.total.saturating_add(n);
        if self.total > LIMITS.total_size {
            return Err(too_large());
        }
        Ok(())
    }
}

fn too_large() -> AppError {
    AppError::new("backup_too_large", "This is too large to back up")
}

fn file_options(len: u64) -> SimpleFileOptions {
    SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .unix_permissions(0o644)
        .large_file(len >= 0xFFFF_FFFF)
}

fn dir_options() -> SimpleFileOptions {
    SimpleFileOptions::default().unix_permissions(0o755)
}

/// Add `root` as `<prefix>/...`. Symlinks are skipped, never followed.
fn add_tree<W: Write + Seek>(
    zw: &mut ZipWriter<W>,
    root: &Path,
    prefix: &str,
    budget: &mut Budget,
) -> Res<()> {
    budget.entry()?;
    zw.add_directory(prefix, dir_options()).map_err(failed)?;
    let mut stack = vec![(root.to_path_buf(), prefix.to_string())];
    while let Some((dir, rel)) = stack.pop() {
        let mut entries: Vec<fs::DirEntry> = fs::read_dir(&dir)?.flatten().collect();
        entries.sort_by_key(fs::DirEntry::file_name);
        for entry in entries {
            let Some(name) = entry.file_name().to_str().map(str::to_string) else {
                tracing::warn!("backup: skipped a non-UTF-8 name in {}", dir.display());
                continue;
            };
            let entry_rel = format!("{rel}/{name}");
            if !zip_name_ok(&entry_rel) {
                tracing::warn!("backup: skipped unsupported name {entry_rel:?}");
                continue;
            }
            let kind = entry.file_type()?;
            if kind.is_symlink() {
                tracing::debug!("backup: skipped symlink {entry_rel}");
            } else if kind.is_dir() {
                budget.entry()?;
                zw.add_directory(&entry_rel, dir_options())
                    .map_err(failed)?;
                stack.push((entry.path(), entry_rel));
            } else if kind.is_file() {
                let len = entry.metadata()?.len();
                budget.entry()?;
                budget.bytes(len)?;
                zw.start_file(&entry_rel, file_options(len))
                    .map_err(failed)?;
                let file = File::open(entry.path())?;
                let n = io::copy(&mut file.take(LIMITS.entry_size.saturating_add(1)), zw)?;
                if n > LIMITS.entry_size {
                    return Err(too_large());
                }
            }
        }
    }
    Ok(())
}

struct NewBackup<'a> {
    profile_id: &'a str,
    kind: BackupKind,
    world_id: Option<String>,
    label: String,
    reason: BackupReason,
}

/// Create `<id>.zip` and `<id>.json` in `dir`. The zip is written under a
/// `.part` name and renamed when complete; nothing is left behind on failure.
fn write_backup(
    dir: &Path,
    nb: NewBackup<'_>,
    fill: impl FnOnce(&mut ZipWriter<&mut File>) -> Res<()>,
) -> Res<Backup> {
    fs::create_dir_all(dir)?;
    let label = file_label(&nb.label);
    let mut ms = now_ms();
    let (id, part, mut file) = loop {
        let id = format!("{ms}-{}-{label}", nb.kind.as_str());
        let part = dir.join(format!("{id}.zip.part"));
        let taken = zip_path(dir, &id).exists() || sidecar_path(dir, &id).exists();
        if !taken {
            match fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&part)
            {
                Ok(f) => break (id, part, f),
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
                Err(e) => return Err(e.into()),
            }
        }
        ms += 1;
    };
    let final_zip = zip_path(dir, &id);
    let sidecar = sidecar_path(dir, &id);
    let result = (|| -> Res<Backup> {
        let mut zw = ZipWriter::new(&mut file);
        fill(&mut zw)?;
        zw.finish().map_err(failed)?;
        file.sync_all()?;
        let size = file.metadata()?.len();
        fs::rename(&part, &final_zip)?;
        let backup = Backup {
            id: id.clone(),
            kind: nb.kind,
            profile_id: nb.profile_id.to_string(),
            world_id: nb.world_id.clone(),
            created_at: ms,
            size_bytes: size,
            label: nb.label.clone(),
            reason: nb.reason,
        };
        let tmp = dir.join(format!("{id}.json.part"));
        fs::write(&tmp, serde_json::to_vec_pretty(&backup)?)?;
        fs::rename(&tmp, &sidecar)?;
        Ok(backup)
    })();
    if result.is_err() {
        for p in [
            &part,
            &final_zip,
            &sidecar,
            &dir.join(format!("{id}.json.part")),
        ] {
            let _ = fs::remove_file(p);
        }
    }
    result
}

/// Zip one world folder (as `<world>/...`).
pub(crate) fn create_world_backup(
    paths: &Paths,
    profile_id: &str,
    world_id: &str,
    label: Option<&str>,
    reason: BackupReason,
) -> Res<Backup> {
    let world = world_path(paths, profile_id, world_id)?;
    let dir = paths.backup_dir(profile_id)?;
    let label = display_label(label.unwrap_or(""), world_id);
    write_backup(
        &dir,
        NewBackup {
            profile_id,
            kind: BackupKind::World,
            world_id: Some(world_id.to_string()),
            label,
            reason,
        },
        |zw| {
            add_tree(
                zw,
                &world,
                world_id,
                &mut Budget {
                    entries: 0,
                    total: 0,
                },
            )
        },
    )
}

/// True when a snapshot of this profile would hold anything.
pub(crate) fn has_snapshot_content(paths: &Paths, profile: &Profile) -> bool {
    if !profile.mods.is_empty() {
        return true;
    }
    paths.instance_dir(&profile.id).is_ok_and(|inst| {
        SNAPSHOT_DIRS.iter().any(|d| {
            let dir = inst.join(d);
            is_real_dir(&dir) && fs::read_dir(&dir).is_ok_and(|mut r| r.next().is_some())
        })
    })
}

/// Zip the content folders plus the mod list.
pub(crate) fn create_snapshot(
    paths: &Paths,
    profile: &Profile,
    label: Option<&str>,
    reason: BackupReason,
) -> Res<Backup> {
    let inst = paths.instance_dir(&profile.id)?;
    let dir = paths.backup_dir(&profile.id)?;
    let label = display_label(label.unwrap_or(""), "Snapshot");
    write_backup(
        &dir,
        NewBackup {
            profile_id: &profile.id,
            kind: BackupKind::Snapshot,
            world_id: None,
            label,
            reason,
        },
        |zw| {
            let mut budget = Budget {
                entries: 0,
                total: 0,
            };
            let mut folders = Vec::new();
            for name in SNAPSHOT_DIRS {
                let src = inst.join(name);
                if is_real_dir(&src) {
                    add_tree(zw, &src, name, &mut budget)?;
                    folders.push(name.to_string());
                }
            }
            let manifest = SnapshotManifest {
                version: 1,
                profile_id: profile.id.clone(),
                name: profile.name.clone(),
                mc_version: profile.mc_version.clone(),
                loader: profile.loader,
                loader_version: profile.loader_version.clone(),
                instance_dir: inst.display().to_string(),
                folders,
                mods: profile.mods.clone(),
                created_at: now_ms(),
            };
            let bytes = serde_json::to_vec_pretty(&manifest)?;
            zw.start_file(MANIFEST, file_options(bytes.len() as u64))
                .map_err(failed)?;
            zw.write_all(&bytes)?;
            Ok(())
        },
    )
}

// ---- listing and deleting ----

fn read_backup(dir: &Path, profile_id: &str, id: &str) -> Option<Backup> {
    let kind = valid_backup_id(id)?;
    let zip_meta = fs::symlink_metadata(zip_path(dir, id)).ok()?;
    if !zip_meta.is_file() {
        return None;
    }
    let side = sidecar_path(dir, id);
    let side_meta = fs::symlink_metadata(&side).ok()?;
    if !side_meta.is_file() || side_meta.len() > MAX_SIDECAR {
        return None;
    }
    let mut backup: Backup = serde_json::from_slice(&fs::read(&side).ok()?).ok()?;
    let world_ok = match kind {
        BackupKind::World => backup.world_id.as_deref().is_some_and(safe_folder_name),
        BackupKind::Snapshot => backup.world_id.is_none(),
    };
    if backup.id != id || backup.profile_id != profile_id || backup.kind != kind || !world_ok {
        return None;
    }
    backup.size_bytes = zip_meta.len();
    Some(backup)
}

/// Newest first; unreadable entries are ignored.
pub(crate) fn list_backups(paths: &Paths, profile_id: &str) -> Res<Vec<Backup>> {
    let dir = paths.backup_dir(profile_id)?;
    let Ok(rd) = fs::read_dir(&dir) else {
        return Ok(Vec::new());
    };
    let mut out: Vec<Backup> = rd
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().into_string().ok()?;
            let id = name.strip_suffix(".json")?;
            read_backup(&dir, profile_id, id)
        })
        .collect();
    out.sort_by(|a, b| {
        b.created_at
            .cmp(&a.created_at)
            .then_with(|| b.id.cmp(&a.id))
    });
    Ok(out)
}

fn delete_backup_files(paths: &Paths, profile_id: &str, id: &str) -> Res<()> {
    if valid_backup_id(id).is_none() {
        return Err(not_found());
    }
    let dir = paths.backup_dir(profile_id)?;
    let (zip, side) = (zip_path(&dir, id), sidecar_path(&dir, id));
    if !zip.exists() && !side.exists() {
        return Err(not_found());
    }
    for p in [&zip, &side] {
        match fs::remove_file(p) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
    }
    Ok(())
}

/// Keep the newest `keep` automatic backups of each kind (manual ones never
/// expire, nor does `protect`). Kinds are pruned separately so a stream of
/// snapshots cannot push out the backup made before a world was deleted.
pub(crate) fn prune_automatic(paths: &Paths, profile_id: &str, keep: u32, protect: Option<&str>) {
    let Ok(all) = list_backups(paths, profile_id) else {
        return;
    };
    for kind in [BackupKind::World, BackupKind::Snapshot] {
        let stale = all
            .iter()
            .filter(|b| b.kind == kind && b.reason != BackupReason::Manual)
            .skip(keep as usize);
        for b in stale {
            if protect == Some(b.id.as_str()) {
                continue;
            }
            if let Err(e) = delete_backup_files(paths, profile_id, &b.id) {
                tracing::warn!("could not prune backup {}: {e}", b.id);
            }
        }
    }
}

// ---- restoring ----

/// Every entry must be a safe relative name accepted by `allowed`.
fn check_entries<R: Read + Seek>(
    zip: &mut ZipArchive<R>,
    allowed: impl Fn(&str) -> bool,
) -> Res<()> {
    if zip.len() > LIMITS.entries {
        return Err(corrupt("too many entries"));
    }
    for i in 0..zip.len() {
        let f = zip.by_index_raw(i).map_err(corrupt)?;
        let name = f.name();
        if !zip_name_ok(name) || !allowed(name) {
            return Err(corrupt(format!("unexpected entry {name:?}")));
        }
        if mango_archive::is_symlink_mode(f.unix_mode()) {
            return Err(corrupt(format!("symlink entry {name:?}")));
        }
    }
    Ok(())
}

fn exists(p: &Path) -> bool {
    fs::symlink_metadata(p).is_ok()
}

/// `base` itself when free (and not `as_new`), else `base (restored)`,
/// `base (restored 2)`, ...
fn restored_name(saves: &Path, base: &str, as_new: bool) -> Res<String> {
    if !as_new && !exists(&saves.join(base)) {
        return Ok(base.to_string());
    }
    for n in 1..1000u32 {
        let suffix = if n == 1 {
            " (restored)".to_string()
        } else {
            format!(" (restored {n})")
        };
        let mut stem = base.to_string();
        while stem.len() + suffix.len() > 255 {
            stem.pop();
        }
        let candidate = format!("{stem}{suffix}");
        if !exists(&saves.join(&candidate)) {
            return Ok(candidate);
        }
    }
    Err(AppError::new(
        "restore_name_taken",
        "No free folder name for the restored world",
    ))
}

/// A fresh sibling folder name for staging.
fn staging_dir(parent: &Path, prefix: &str) -> PathBuf {
    let mut n = now_ms();
    loop {
        let p = parent.join(format!("{prefix}{n}"));
        if !exists(&p) {
            return p;
        }
        n += 1;
    }
}

/// Extract a world backup as a new folder of `saves/`. Returns its name.
pub(crate) fn restore_world(
    paths: &Paths,
    profile_id: &str,
    backup: &Backup,
    as_new: bool,
) -> Res<String> {
    let world_id = backup
        .world_id
        .as_deref()
        .filter(|w| safe_folder_name(w))
        .ok_or_else(|| corrupt("no world name"))?;
    let dir = paths.backup_dir(profile_id)?;
    let archive = zip_path(&dir, &backup.id);
    let prefix = format!("{world_id}/");
    let mut zip = ZipArchive::new(File::open(&archive)?).map_err(corrupt)?;
    check_entries(&mut zip, |n| n.starts_with(&prefix))?;
    drop(zip);

    let saves = crate::worlds::saves_dir(paths, profile_id)?;
    if exists(&saves) && !is_real_dir(&saves) {
        return Err(AppError::new(
            "invalid_world",
            "The saves folder is not a folder",
        ));
    }
    fs::create_dir_all(&saves)?;
    let target = restored_name(&saves, world_id, as_new)?;
    let staging = staging_dir(&saves, ".mango-restore-");
    let result = (|| -> Res<()> {
        extract_zip(File::open(&archive)?, &staging, &LIMITS).map_err(archive_error)?;
        let extracted = staging.join(world_id);
        if !is_real_dir(&extracted) {
            return Err(corrupt("the world folder is missing"));
        }
        // `rename` onto an existing non-empty folder fails: never overwrites.
        fs::rename(&extracted, saves.join(&target))?;
        Ok(())
    })();
    let _ = fs::remove_dir_all(&staging);
    result?;
    Ok(target)
}

/// Move the four content folders out of the way and the snapshot's in; roll
/// back on failure.
fn swap_dirs(inst: &Path, staging: &Path, old: &Path) -> io::Result<()> {
    fs::create_dir_all(old)?;
    let mut moved_old: Vec<&str> = Vec::new();
    let mut placed: Vec<&str> = Vec::new();
    let result = (|| -> io::Result<()> {
        for name in SNAPSHOT_DIRS {
            let current = inst.join(name);
            if exists(&current) {
                fs::rename(&current, old.join(name))?;
                moved_old.push(name);
            }
        }
        for name in SNAPSHOT_DIRS {
            let staged = staging.join(name);
            if is_real_dir(&staged) {
                fs::rename(&staged, inst.join(name))?;
                placed.push(name);
            }
        }
        Ok(())
    })();
    if result.is_err() {
        for name in placed.iter().rev() {
            let _ = fs::remove_dir_all(inst.join(name));
        }
        for name in moved_old.iter().rev() {
            let _ = fs::rename(old.join(name), inst.join(name));
        }
    }
    result
}

/// Put the snapshot's folders back (replacing the current ones). Returns the
/// stored profile record; the caller restores the mod list.
pub(crate) fn restore_snapshot_files(
    paths: &Paths,
    profile_id: &str,
    backup: &Backup,
) -> Res<SnapshotManifest> {
    let dir = paths.backup_dir(profile_id)?;
    let archive = zip_path(&dir, &backup.id);
    let mut zip = ZipArchive::new(File::open(&archive)?).map_err(corrupt)?;
    check_entries(&mut zip, |n| {
        n == MANIFEST
            || SNAPSHOT_DIRS
                .iter()
                .any(|d| n == *d || n.starts_with(&format!("{d}/")))
    })?;
    let manifest: SnapshotManifest = {
        let f = zip.by_name(MANIFEST).map_err(corrupt)?;
        let mut raw = Vec::new();
        f.take(MAX_MANIFEST + 1).read_to_end(&mut raw)?;
        if raw.len() as u64 > MAX_MANIFEST {
            return Err(corrupt("profile record too large"));
        }
        serde_json::from_slice(&raw).map_err(corrupt)?
    };
    drop(zip);

    let inst = paths.instance_dir(profile_id)?;
    fs::create_dir_all(&inst)?;
    let staging = staging_dir(&inst, ".mango-restore-");
    let old = staging_dir(&inst, ".mango-old-");
    let result = (|| -> Res<()> {
        extract_zip(File::open(&archive)?, &staging, &LIMITS).map_err(archive_error)?;
        swap_dirs(&inst, &staging, &old)?;
        Ok(())
    })();
    let _ = fs::remove_dir_all(&staging);
    if result.is_ok() {
        let _ = fs::remove_dir_all(&old);
    } else if is_real_dir(&old) && fs::read_dir(&old).is_ok_and(|mut r| r.next().is_none()) {
        let _ = fs::remove_dir(&old);
    }
    result?;
    Ok(manifest)
}

// ---- the service ----

/// Marks the profile as busy (like a launch) so the game cannot start while
/// its files are being replaced.
struct Reserve {
    inner: Arc<Inner>,
    id: String,
}

impl Drop for Reserve {
    fn drop(&mut self) {
        lock(&self.inner.launching).remove(&self.id);
    }
}

impl AppService {
    fn reserve(&self, profile_id: &str) -> Res<Reserve> {
        if lock(&self.inner.games).contains_key(profile_id)
            || !lock(&self.inner.launching).insert(profile_id.to_string())
        {
            return Err(game_running());
        }
        Ok(Reserve {
            inner: Arc::clone(&self.inner),
            id: profile_id.to_string(),
        })
    }

    fn backup_keep(&self) -> u32 {
        self.inner.with_store(|s| s.config.backup_keep)
    }

    /// Zip one world. Allowed while the game runs (a copy is not a change).
    pub async fn world_backup(
        &self,
        profile_id: String,
        world_id: String,
        label: Option<String>,
    ) -> Res<Backup> {
        self.profile(&profile_id)?;
        let inner = Arc::clone(&self.inner);
        tokio::task::spawn_blocking(move || {
            let _gate = lock(&inner.backup_gate);
            create_world_backup(
                &inner.paths,
                &profile_id,
                &world_id,
                label.as_deref(),
                BackupReason::Manual,
            )
        })
        .await?
    }

    /// Delete a world folder, backing it up first when asked to. The folder
    /// is only removed after the backup succeeded.
    pub async fn world_delete(
        &self,
        profile_id: String,
        world_id: String,
        make_backup: bool,
    ) -> Res<Option<Backup>> {
        self.profile(&profile_id)?;
        let reserve = self.reserve(&profile_id)?;
        let inner = Arc::clone(&self.inner);
        let keep = self.backup_keep();
        tokio::task::spawn_blocking(move || {
            let _reserve = reserve;
            let _gate = lock(&inner.backup_gate);
            let world = world_path(&inner.paths, &profile_id, &world_id)?;
            let backup = if make_backup {
                let label = format!("Before deleting {world_id}");
                let b = create_world_backup(
                    &inner.paths,
                    &profile_id,
                    &world_id,
                    Some(&label),
                    BackupReason::BeforeDelete,
                )?;
                prune_automatic(&inner.paths, &profile_id, keep, Some(&b.id));
                Some(b)
            } else {
                None
            };
            fs::remove_dir_all(&world)?;
            Ok(backup)
        })
        .await?
    }

    pub async fn backups_list(&self, profile_id: String) -> Res<Vec<Backup>> {
        self.profile(&profile_id)?;
        let paths = self.inner.paths.clone();
        tokio::task::spawn_blocking(move || list_backups(&paths, &profile_id)).await?
    }

    pub async fn backup_delete(&self, profile_id: String, backup_id: String) -> Res<Vec<Backup>> {
        self.profile(&profile_id)?;
        let inner = Arc::clone(&self.inner);
        tokio::task::spawn_blocking(move || {
            let _gate = lock(&inner.backup_gate);
            delete_backup_files(&inner.paths, &profile_id, &backup_id)?;
            list_backups(&inner.paths, &profile_id)
        })
        .await?
    }

    /// Restore a world as a new folder, or a snapshot over the current
    /// content folders and mod list. Refused while the game runs.
    pub async fn backup_restore(
        &self,
        profile_id: String,
        backup_id: String,
        as_new: Option<bool>,
    ) -> Res<Vec<Backup>> {
        let profile = self.profile(&profile_id)?;
        let reserve = self.reserve(&profile_id)?;
        let inner = Arc::clone(&self.inner);
        let keep = self.backup_keep();
        let as_new = as_new.unwrap_or(false);
        let pid = profile_id.clone();
        let (list, manifest) = tokio::task::spawn_blocking(move || -> Res<_> {
            let profile_id = pid;
            let _reserve = reserve;
            let _gate = lock(&inner.backup_gate);
            let paths = &inner.paths;
            let dir = paths.backup_dir(&profile_id)?;
            if valid_backup_id(&backup_id).is_none() {
                return Err(not_found());
            }
            let backup = read_backup(&dir, &profile_id, &backup_id).ok_or_else(not_found)?;
            let manifest = match backup.kind {
                BackupKind::World => {
                    restore_world(paths, &profile_id, &backup, as_new)?;
                    None
                }
                BackupKind::Snapshot => {
                    // The current state first, so a restore can be undone. If
                    // that fails nothing is touched.
                    create_snapshot(
                        paths,
                        &profile,
                        Some("Before restoring a snapshot"),
                        BackupReason::Auto,
                    )?;
                    let m = restore_snapshot_files(paths, &profile_id, &backup)?;
                    prune_automatic(paths, &profile_id, keep, Some(&backup.id));
                    Some(m)
                }
            };
            Ok((list_backups(paths, &profile_id)?, manifest))
        })
        .await??;

        if let Some(m) = manifest {
            let inst = self.inner.paths.instance_dir(&profile_id)?;
            let mods = rebase_records(m.mods, &m.instance_dir, &inst);
            self.save_mods(&profile_id, &mods)?;
            self.inner
                .emit(Event::ContentChanged(ContentChangedEvent { profile_id }));
        }
        Ok(list)
    }

    /// A manual snapshot of the content folders and the mod list.
    pub async fn snapshot_create(&self, profile_id: String, label: Option<String>) -> Res<Backup> {
        let profile = self.profile(&profile_id)?;
        let inner = Arc::clone(&self.inner);
        tokio::task::spawn_blocking(move || {
            let _gate = lock(&inner.backup_gate);
            create_snapshot(
                &inner.paths,
                &profile,
                label.as_deref(),
                BackupReason::Manual,
            )
        })
        .await?
    }

    /// The safety net of destructive content changes: a snapshot with
    /// `reason`, when `autoSnapshot` is on and there is something to keep.
    /// Never fails and never blocks the action: a failure is logged and
    /// reported as a warning line on `launch:log` (and the action goes on).
    pub(crate) async fn auto_snapshot(
        &self,
        profile_id: &str,
        reason: BackupReason,
        label: String,
    ) -> Option<Backup> {
        let (enabled, keep) = self
            .inner
            .with_store(|s| (s.config.auto_snapshot, s.config.backup_keep));
        if !enabled {
            return None;
        }
        let profile = self.profile(profile_id).ok()?;
        let inner = Arc::clone(&self.inner);
        let outcome = tokio::task::spawn_blocking(move || -> Res<Option<Backup>> {
            let _gate = lock(&inner.backup_gate);
            if !has_snapshot_content(&inner.paths, &profile) {
                return Ok(None);
            }
            let b = create_snapshot(&inner.paths, &profile, Some(&label), reason)?;
            prune_automatic(&inner.paths, &profile.id, keep, None);
            Ok(Some(b))
        })
        .await;
        match outcome {
            Ok(Ok(backup)) => backup,
            Ok(Err(e)) => {
                self.snapshot_failed(profile_id, &e.to_string());
                None
            }
            Err(e) => {
                self.snapshot_failed(profile_id, &e.to_string());
                None
            }
        }
    }

    fn snapshot_failed(&self, profile_id: &str, why: &str) {
        tracing::warn!("automatic snapshot of {profile_id} failed: {why}");
        self.warn(
            profile_id,
            format!("Could not take the automatic snapshot first ({why}); continuing without it"),
        );
    }
}

#[cfg(test)]
pub(crate) fn rebase_records_for_test(
    mods: Vec<ModRecord>,
    old_dir: &str,
    new_dir: &Path,
) -> Vec<ModRecord> {
    rebase_records(mods, old_dir, new_dir)
}

/// Re-base `file` paths recorded under `old_dir` onto `new_dir`.
fn rebase_records(mods: Vec<ModRecord>, old_dir: &str, new_dir: &Path) -> Vec<ModRecord> {
    mods.into_iter()
        .map(|mut m| {
            if let Some(file) = m.file.as_deref() {
                match Path::new(file).strip_prefix(old_dir) {
                    // Only plain relative components: a tampered sidecar must
                    // not rebase a record out of the instance.
                    Ok(rel)
                        if rel
                            .components()
                            .all(|c| matches!(c, std::path::Component::Normal(_))) =>
                    {
                        m.file = Some(new_dir.join(rel).display().to_string());
                    }
                    Ok(_) => m.file = None,
                    Err(_) => {}
                }
            }
            m
        })
        .collect()
}
