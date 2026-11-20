//! Reconcile `profile.mods` with the instance `mods/` folder.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use mango_core::model::{ModRecord, ModType, Profile};
use mango_core::paths::Paths;
use regex::Regex;

use crate::error::ContentError;
use crate::meta::{read_jar_meta, JarMeta};
use crate::perf::managed_names;

/// A jar found in the mods folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JarFile {
    pub file: PathBuf,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SyncResult {
    pub mods: Vec<ModRecord>,
    pub changed: bool,
}

pub fn mods_dir(paths: &Paths, profile_id: &str) -> Result<PathBuf, ContentError> {
    Ok(paths.instance_dir(profile_id)?.join("mods"))
}

fn is_jar(name: &str) -> bool {
    let l = name.to_ascii_lowercase();
    l.ends_with(".jar") || l.ends_with(".jar.disabled")
}

fn is_disabled(name: &str) -> bool {
    name.to_ascii_lowercase().ends_with(".disabled")
}

/// `foo.jar.disabled` -> `foo.jar`, so a mod keeps its identity when toggled.
pub fn jar_name(name: &str) -> String {
    if is_disabled(name) {
        name[..name.len() - ".disabled".len()].to_string()
    } else {
        name.to_string()
    }
}

/// MangoConfig's own jars (`^MangoConfig-.*\.jar(\.disabled)?$`).
pub fn is_own_mango_jar(name: &str) -> bool {
    static R: OnceLock<Option<Regex>> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"^MangoConfig-.*\.jar(\.disabled)?$").ok())
        .as_ref()
        .is_some_and(|r| r.is_match(name))
}

/// Map of `<name>.jar` -> jar for everything the player manages in `dir`.
/// A missing folder yields an empty map.
pub fn scan_mods_dir(dir: &Path) -> BTreeMap<String, JarFile> {
    let mut found: BTreeMap<String, JarFile> = BTreeMap::new();
    let Ok(rd) = std::fs::read_dir(dir) else {
        return found;
    };
    let managed = managed_names(dir);
    let mut entries: Vec<_> = rd.flatten().collect();
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !entry.file_type().is_ok_and(|t| t.is_file()) || !is_jar(&name) {
            continue;
        }
        if is_own_mango_jar(&name) || managed.contains(&name) {
            continue;
        }
        let key = jar_name(&name);
        let enabled = !is_disabled(&name);
        // If both `foo.jar` and `foo.jar.disabled` exist, the live one wins.
        if found.get(&key).is_some_and(|j| j.enabled) {
            continue;
        }
        found.insert(
            key,
            JarFile {
                file: dir.join(&name),
                enabled,
            },
        );
    }
    found
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

fn icon_dir(paths: &Paths) -> PathBuf {
    paths.cache().join("mod-icons")
}

fn local_record(paths: &Paths, filename: &str, jar: &JarFile) -> ModRecord {
    let meta: JarMeta = read_jar_meta(&jar.file, &icon_dir(paths));
    ModRecord {
        project_id: Some(format!("local:{filename}")),
        slug: None,
        title: Some(meta.title),
        kind: Some(ModType::Mod),
        icon: meta.icon,
        version_id: None,
        version_number: Some(meta.version),
        filename: Some(filename.to_string()),
        file: Some(jar.file.to_string_lossy().into_owned()),
        game_versions: Some(Vec::new()),
        loaders: Some(Vec::new()),
        installed_at: Some(now_ms()),
        dependency: Some(false),
        local: Some(true),
        enabled: Some(jar.enabled),
        ..ModRecord::default()
    }
}

/// Reconcile a profile's mod list with its `mods` folder. Returns the list to
/// store and whether anything moved. Never drops a record for a non-mod type
/// unless its own file is gone.
pub fn sync_profile_mods(paths: &Paths, profile: &Profile) -> Result<SyncResult, ContentError> {
    let dir = mods_dir(paths, &profile.id)?;
    let mut on_disk = scan_mods_dir(&dir);
    let mut mods = Vec::new();
    let mut changed = false;

    for rec in &profile.mods {
        if rec.kind.unwrap_or(ModType::Mod) != ModType::Mod {
            match &rec.file {
                Some(f) if !Path::new(f).exists() => changed = true,
                _ => mods.push(rec.clone()),
            }
            continue;
        }
        let filename = rec
            .filename
            .clone()
            .filter(|f| !f.is_empty())
            .or_else(|| {
                rec.file
                    .as_deref()
                    .and_then(|f| Path::new(f).file_name())
                    .map(|n| jar_name(&n.to_string_lossy()))
            })
            .unwrap_or_default();
        let Some(hit) = on_disk.remove(&filename) else {
            changed = true; // jar deleted outside the launcher
            continue;
        };
        let hit_file = hit.file.to_string_lossy().into_owned();
        let inline_icon = rec.local == Some(true)
            && rec
                .icon
                .as_deref()
                .is_some_and(|i| i.starts_with("data:image/"));
        if rec.file.as_deref() != Some(hit_file.as_str())
            || rec.enabled.unwrap_or(true) != hit.enabled
            || inline_icon
        {
            changed = true;
            let mut next = rec.clone();
            if inline_icon {
                next.icon = read_jar_meta(&hit.file, &icon_dir(paths)).icon;
            }
            next.filename = Some(filename);
            next.file = Some(hit_file);
            next.enabled = Some(hit.enabled);
            mods.push(next);
        } else {
            mods.push(rec.clone());
        }
    }

    // Whatever is left was copied in by hand.
    let mut added: Vec<_> = on_disk.into_iter().collect();
    added.sort_by_key(|(name, _)| (name.to_lowercase(), name.clone()));
    for (filename, jar) in added {
        changed = true;
        mods.push(local_record(paths, &filename, &jar));
    }
    Ok(SyncResult { mods, changed })
}

/// Enable or disable a local mod by renaming its jar with `.disabled`.
/// Returns the updated record.
pub fn set_local_mod_enabled(rec: &ModRecord, enabled: bool) -> Result<ModRecord, ContentError> {
    if rec.local != Some(true) {
        return Err(ContentError::NotLocal);
    }
    let file = rec.file.as_deref().ok_or(ContentError::NoFile)?;
    let current = PathBuf::from(file);
    let live = PathBuf::from(jar_name(file));
    let disabled = {
        let mut s = live.clone().into_os_string();
        s.push(".disabled");
        PathBuf::from(s)
    };
    let target = if enabled { live } else { disabled };
    if current != target {
        if current.exists() {
            std::fs::rename(&current, &target).map_err(|e| ContentError::io(&current, e))?;
        } else if !target.exists() {
            return Err(ContentError::io(
                &current,
                std::io::Error::from(std::io::ErrorKind::NotFound),
            ));
        }
    }
    let mut next = rec.clone();
    next.file = Some(target.to_string_lossy().into_owned());
    next.enabled = Some(enabled);
    Ok(next)
}
