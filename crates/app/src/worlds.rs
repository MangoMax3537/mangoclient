//! Part 9 of `docs/ipc.md`: the worlds (`saves/*`) of an instance.
//!
//! A world is a folder under `instances/<id>/saves/` that holds a
//! `level.dat`. Its metadata comes from that gzip NBT file (parsed with the
//! NBT reader of `mango-servers`); the folder name is the stable id.

use crate::error::{AppError, Res};
use crate::service::AppService;
use mango_core::Paths;
use mango_servers::nbt::{self, Entry, Value};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use ts_rs::TS;

/// A `level.dat` larger than this is not read (a real one is a few KiB).
const MAX_LEVEL_DAT: u64 = 16 * 1024 * 1024;
/// `icon.png` of a world is 64x64; anything bigger is not served.
pub const MAX_WORLD_ICON: u64 = 2 * 1024 * 1024;
/// Folder size walk: stop counting after this many entries or this long.
const WALK_MAX_ENTRIES: u64 = 400_000;
const WALK_MAX_TIME: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "lowercase")]
pub enum GameMode {
    Survival,
    Creative,
    Adventure,
    Spectator,
}

impl GameMode {
    fn from_id(id: i64) -> Option<GameMode> {
        Some(match id {
            0 => GameMode::Survival,
            1 => GameMode::Creative,
            2 => GameMode::Adventure,
            3 => GameMode::Spectator,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct World {
    /// The folder name.
    pub id: String,
    /// `LevelName`, else the folder name.
    pub name: String,
    /// Epoch ms; `null` when unknown.
    pub last_played: Option<u64>,
    pub size_bytes: u64,
    pub game_mode: Option<GameMode>,
    pub hardcore: bool,
    /// `Version.Name`, e.g. `1.21.1`.
    pub version: Option<String>,
    /// `mangoimg://world/<profileId>/<id>` when the world has an `icon.png`.
    pub icon_url: Option<String>,
}

/// What `level.dat` says; every field may be missing.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct LevelInfo {
    pub name: Option<String>,
    pub last_played: Option<u64>,
    pub game_mode: Option<GameMode>,
    pub hardcore: bool,
    pub version: Option<String>,
}

fn as_int(v: &Value) -> Option<i64> {
    match v {
        Value::Byte(n) => Some(i64::from(*n)),
        Value::Short(n) => Some(i64::from(*n)),
        Value::Int(n) => Some(i64::from(*n)),
        Value::Long(n) => Some(*n),
        _ => None,
    }
}

fn compound<'a>(entries: &'a [Entry], name: &str) -> Option<&'a [Entry]> {
    match &nbt::field(entries, name)?.value {
        Value::Compound(c) => Some(c),
        _ => None,
    }
}

/// Read the interesting fields out of the bytes of a `level.dat`
/// (gzip-compressed or raw NBT). Unknown or mistyped fields are left empty.
pub(crate) fn parse_level(bytes: &[u8]) -> Option<LevelInfo> {
    let root = nbt::parse_nbt_maybe_gzip(bytes).ok()?;
    let data = compound(&root.entries, "Data")?;
    let int = |name: &str| nbt::field(data, name).and_then(|e| as_int(&e.value));
    let name = nbt::field(data, "LevelName")
        .and_then(|e| e.value.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let version = compound(data, "Version")
        .and_then(|v| nbt::field(v, "Name"))
        .and_then(|e| e.value.as_str())
        .map(str::to_string)
        .filter(|s| !s.is_empty());
    Some(LevelInfo {
        name,
        last_played: int("LastPlayed").and_then(|n| u64::try_from(n).ok().filter(|n| *n > 0)),
        game_mode: int("GameType").and_then(GameMode::from_id),
        hardcore: int("hardcore").is_some_and(|n| n != 0),
        version,
    })
}

/// A folder name that is one plain path component and safe on every OS.
pub(crate) fn safe_folder_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 255
        && !name.starts_with('.')
        && !name.chars().any(|c| {
            c.is_control() || matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|')
        })
}

/// Percent-encode everything but unreserved characters (one URL segment).
pub(crate) fn encode_segment(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(char::from(b));
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// Inverse of [`encode_segment`]; `None` for malformed escapes or bad UTF-8.
pub(crate) fn decode_segment(s: &str) -> Option<String> {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' {
            let hex = s.get(i + 1..i + 3)?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

/// `mangoimg://world/<profileId>/<folder>`.
pub fn world_icon_url(profile_id: &str, folder: &str) -> String {
    format!("mangoimg://world/{profile_id}/{}", encode_segment(folder))
}

/// `true` for a real directory (a symlink to one does not count).
fn is_real_dir(p: &Path) -> bool {
    std::fs::symlink_metadata(p).is_ok_and(|m| m.is_dir())
}

fn is_real_file(p: &Path) -> bool {
    std::fs::symlink_metadata(p).is_ok_and(|m| m.is_file())
}

/// `<instance>/saves`, which must be a real directory when it exists.
pub(crate) fn saves_dir(paths: &Paths, profile_id: &str) -> Res<PathBuf> {
    Ok(paths.instance_dir(profile_id)?.join("saves"))
}

/// The folder of one world, checked: plain name, real directory, no symlinks
/// on the way (`saves` itself included).
pub(crate) fn world_path(paths: &Paths, profile_id: &str, world_id: &str) -> Res<PathBuf> {
    if !safe_folder_name(world_id) {
        return Err(AppError::new("invalid_world", "Invalid world folder name"));
    }
    let saves = saves_dir(paths, profile_id)?;
    let dir = saves.join(world_id);
    if !is_real_dir(&saves) || !is_real_dir(&dir) {
        return Err(AppError::new("world_not_found", "World not found"));
    }
    Ok(dir)
}

/// What a bounded folder walk found.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Walk {
    pub bytes: u64,
    pub files: u64,
    /// The walk stopped early (entry or time cap); `bytes` is a lower bound.
    pub truncated: bool,
}

/// Sum of the file sizes below `root`. Symlinks are neither followed nor
/// counted.
pub(crate) fn dir_size(root: &Path, max_entries: u64, max_time: Duration) -> Walk {
    let started = Instant::now();
    let mut walk = Walk::default();
    let mut seen = 0u64;
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in rd.flatten() {
            seen += 1;
            if seen > max_entries || (seen.is_multiple_of(256) && started.elapsed() > max_time) {
                walk.truncated = true;
                return walk;
            }
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if kind.is_dir() {
                stack.push(entry.path());
            } else if kind.is_file() {
                walk.files += 1;
                walk.bytes = walk
                    .bytes
                    .saturating_add(entry.metadata().map_or(0, |m| m.len()));
            }
        }
    }
    walk
}

fn read_level(dir: &Path) -> Option<LevelInfo> {
    let file = dir.join("level.dat");
    let meta = std::fs::symlink_metadata(&file).ok()?;
    if !meta.is_file() || meta.len() > MAX_LEVEL_DAT {
        return None;
    }
    parse_level(&std::fs::read(&file).ok()?)
}

/// Every world of the instance, most recently played first.
pub(crate) fn list_worlds(paths: &Paths, profile_id: &str) -> Res<Vec<World>> {
    let saves = saves_dir(paths, profile_id)?;
    if !is_real_dir(&saves) {
        return Ok(Vec::new());
    }
    let mut worlds = Vec::new();
    for entry in std::fs::read_dir(&saves)?.flatten() {
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        let Some(id) = entry.file_name().to_str().map(str::to_string) else {
            continue;
        };
        if !kind.is_dir() || !safe_folder_name(&id) {
            continue;
        }
        let dir = entry.path();
        // Folders without a level.dat are not worlds (Minecraft skips them too).
        if !is_real_file(&dir.join("level.dat")) {
            continue;
        }
        let info = read_level(&dir).unwrap_or_default();
        let icon = dir.join("icon.png");
        let has_icon = std::fs::symlink_metadata(&icon)
            .is_ok_and(|m| m.is_file() && m.len() <= MAX_WORLD_ICON);
        worlds.push(World {
            name: info.name.unwrap_or_else(|| id.clone()),
            last_played: info.last_played,
            size_bytes: dir_size(&dir, WALK_MAX_ENTRIES, WALK_MAX_TIME).bytes,
            game_mode: info.game_mode,
            hardcore: info.hardcore,
            version: info.version,
            icon_url: has_icon.then(|| world_icon_url(profile_id, &id)),
            id,
        });
    }
    worlds.sort_by(|a, b| {
        b.last_played
            .cmp(&a.last_played)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
            .then_with(|| a.id.cmp(&b.id))
    });
    Ok(worlds)
}

impl AppService {
    /// The worlds in `saves/`, newest first. Never fails for a damaged
    /// `level.dat`: that world is listed under its folder name.
    pub async fn worlds_list(&self, profile_id: String) -> Res<Vec<World>> {
        self.profile(&profile_id)?;
        let paths = self.inner.paths.clone();
        tokio::task::spawn_blocking(move || list_worlds(&paths, &profile_id)).await?
    }

    /// Returns the `saves` path; opens it in the OS file manager when enabled.
    pub fn world_open_folder(&self, profile_id: String) -> Res<String> {
        self.profile(&profile_id)?;
        let dir = saves_dir(&self.inner.paths, &profile_id)?;
        std::fs::create_dir_all(&dir)?;
        self.open_dir(&dir);
        Ok(dir.display().to_string())
    }
}
