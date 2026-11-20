//! config.json + accounts.json (accounts AND profiles) store.

use std::path::{Component, Path, PathBuf};

use serde_json::{json, Value};

use crate::error::{Error, Result};
use crate::model::{total_ram_mb, Account, Config, ModRecord, Profile};
use crate::paths::{validate_profile_id, Paths};
use crate::persistence::{read_json_value, write_json_atomic};
use crate::validation::profile_patch;

const CONTENT_DIRS: [&str; 4] = ["mods", "shaderpacks", "resourcepacks", "datapacks"];

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Lexical normalisation (`.`/`..` resolved without touching the disk).
fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in path.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// Check that a mod record points inside its own instance's content dirs and
/// does not go through a symlink. Returns the record with a normalised path.
fn safe_mod(paths: &Paths, profile_id: &str, mut m: ModRecord) -> Result<ModRecord> {
    if let Some(name) = &m.filename {
        let ok = Path::new(name).file_name().map(|f| f == name.as_str()) == Some(true);
        if !ok {
            return Err(Error::UnsafeMod("Invalid mod filename".into()));
        }
    }
    let Some(file) = m.file.clone() else {
        return Ok(m);
    };
    let base = normalize(&paths.instance_dir(profile_id)?);
    let path = PathBuf::from(&file);
    if !path.is_absolute() {
        return Err(Error::UnsafeMod("Mod file must be absolute".into()));
    }
    let target = normalize(&path);
    let rel = target
        .strip_prefix(&base)
        .map_err(|_| Error::UnsafeMod("Mod file is outside its profile".into()))?;
    let first = rel.components().next().and_then(|c| match c {
        Component::Normal(s) => s.to_str(),
        _ => None,
    });
    if !first.is_some_and(|f| CONTENT_DIRS.contains(&f)) {
        return Err(Error::UnsafeMod("Mod file is outside its profile".into()));
    }
    match std::fs::symlink_metadata(&target) {
        Ok(meta) => {
            if meta.file_type().is_symlink() {
                return Err(Error::UnsafeMod("Mod file cannot be a symlink".into()));
            }
            let real = std::fs::canonicalize(&target).map_err(|e| Error::io(&target, e))?;
            let real_base = std::fs::canonicalize(&base).unwrap_or(base);
            if !real.starts_with(&real_base) {
                return Err(Error::UnsafeMod("Mod file escapes its profile".into()));
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(Error::io(&target, e)),
    }
    m.file = Some(target.to_string_lossy().into_owned());
    Ok(m)
}

fn safe_mods(paths: &Paths, id: &str, mods: Vec<ModRecord>) -> Result<Vec<ModRecord>> {
    mods.into_iter().map(|m| safe_mod(paths, id, m)).collect()
}

#[derive(Debug)]
pub struct Store {
    pub paths: Paths,
    pub config: Config,
    pub accounts: Vec<Account>,
    pub profiles: Vec<Profile>,
    /// Non-fatal problems met while loading (dropped records etc.).
    pub warnings: Vec<String>,
}

impl Store {
    pub fn open(paths: Paths) -> Result<Store> {
        paths.ensure_dirs()?;
        let total = total_ram_mb();
        let config = match read_json_value(&paths.config_file()) {
            Some(raw) => Config::from_value(&raw, total),
            None => Config::with_total_ram(total),
        };
        let raw = read_json_value(&paths.accounts_file()).unwrap_or_else(|| json!({}));
        let mut warnings = Vec::new();

        let accounts = raw
            .get("accounts")
            .and_then(Value::as_array)
            .map(|list| {
                list.iter()
                    .filter_map(|a| match serde_json::from_value::<Account>(a.clone()) {
                        Ok(a) => Some(a),
                        Err(e) => {
                            warnings.push(format!("Ignoring unreadable account: {e}"));
                            None
                        }
                    })
                    .collect()
            })
            .unwrap_or_default();

        let mut profiles = Vec::new();
        for item in raw
            .get("profiles")
            .and_then(Value::as_array)
            .map(Vec::as_slice)
            .unwrap_or(&[])
        {
            // A non-canonical id cannot name an instance dir: skip it.
            let Some(mut p) = Profile::from_stored(item, now_ms()) else {
                continue;
            };
            if validate_profile_id(&p.id).is_err() {
                warnings.push(format!("Ignoring profile with invalid id {}", p.id));
                continue;
            }
            // One stale or unsafe legacy mod path must not discard the profile
            // that owns the player's worlds.
            let id = p.id.clone();
            p.mods = std::mem::take(&mut p.mods)
                .into_iter()
                .filter_map(|m| match safe_mod(&paths, &id, m) {
                    Ok(m) => Some(m),
                    Err(e) => {
                        warnings.push(format!("Ignoring unsafe content in {id}: {e}"));
                        None
                    }
                })
                .collect();
            // The picture file is the source of truth for `cover`.
            p.cover = paths.cover_file(&id)?.exists();
            profiles.push(p);
        }

        let mut store = Store {
            paths,
            config,
            accounts,
            profiles,
            warnings,
        };
        if store.profiles.is_empty() {
            store.create_default_profile()?;
        }
        Ok(store)
    }

    fn create_default_profile(&mut self) -> Result<()> {
        let p = self.add_profile(&json!({
            "name": "MangoClient 1.21.11",
            "mcVersion": "1.21.11",
            "loader": "fabric",
        }))?;
        self.config.selected_profile = Some(p.id);
        self.save_config()
    }

    pub fn save_config(&self) -> Result<()> {
        write_json_atomic(&self.paths.config_file(), &self.config)
    }

    /// accounts.json holds both accounts and profiles.
    pub fn save_accounts(&self) -> Result<()> {
        write_json_atomic(
            &self.paths.accounts_file(),
            &json!({"accounts": self.accounts, "profiles": self.profiles}),
        )
    }

    pub fn set_config(&mut self, patch: &Value) -> Result<&Config> {
        self.config.apply_patch(patch)?;
        self.save_config()?;
        Ok(&self.config)
    }

    // ---- profiles ----

    pub fn add_profile(&mut self, data: &Value) -> Result<Profile> {
        let mut candidate = data.clone();
        if let Some(o) = candidate.as_object_mut() {
            o.remove("id");
            o.remove("created");
        }
        let clean = profile_patch(&candidate)?;
        let mut p = Profile::new(uuid::Uuid::new_v4().to_string(), now_ms());
        p.apply_clean_patch(clean)?;
        if p.name.is_empty() {
            p.name = "New Profile".into();
        }
        p.mods = safe_mods(&self.paths, &p.id, std::mem::take(&mut p.mods))?;
        let dir = self.paths.instance_dir(&p.id)?.join("mods");
        std::fs::create_dir_all(&dir).map_err(|e| Error::io(&dir, e))?;
        self.profiles.push(p.clone());
        self.save_accounts()?;
        Ok(p)
    }

    pub fn update_profile(&mut self, id: &str, patch: &Value) -> Result<Option<&Profile>> {
        let Some(idx) = self.profiles.iter().position(|p| p.id == id) else {
            return Ok(None);
        };
        let mut clean = profile_patch(patch)?;
        if clean.contains_key("mods") {
            let mods: Vec<ModRecord> =
                serde_json::from_value(clean.remove("mods").unwrap_or(Value::Null))?;
            let mods = safe_mods(&self.paths, id, mods)?;
            clean.insert("mods".into(), serde_json::to_value(mods)?);
        }
        self.profiles[idx].apply_clean_patch(clean)?;
        self.save_accounts()?;
        Ok(self.profiles.get(idx))
    }

    pub fn delete_profile(&mut self, id: &str) -> Result<()> {
        let instance = self.paths.instance_dir(id)?;
        let cover = self.paths.cover_file(id)?;
        self.profiles.retain(|p| p.id != id);
        if self.config.selected_profile.as_deref() == Some(id) {
            self.config.selected_profile = self.profiles.first().map(|p| p.id.clone());
            self.save_config()?;
        }
        self.save_accounts()?;
        let _ = std::fs::remove_dir_all(instance);
        let _ = std::fs::remove_file(cover);
        let _ = std::fs::remove_file(self.paths.background_file(id)?);
        let _ = std::fs::remove_file(self.paths.pack_logo_file(id)?);
        Ok(())
    }

    pub fn get_profile(&self, id: &str) -> Option<&Profile> {
        self.profiles.iter().find(|p| p.id == id)
    }

    pub fn selected_profile(&self) -> Option<&Profile> {
        self.config
            .selected_profile
            .as_deref()
            .and_then(|id| self.get_profile(id))
            .or_else(|| self.profiles.first())
    }

    // ---- accounts (plain records) ----

    pub fn upsert_account(&mut self, account: Account) -> Result<()> {
        let uuid = account.uuid.clone();
        match self.accounts.iter_mut().find(|a| a.uuid == uuid) {
            Some(slot) => *slot = account,
            None => self.accounts.push(account),
        }
        if self.config.selected_account.is_none() {
            self.config.selected_account = Some(uuid);
            self.save_config()?;
        }
        self.save_accounts()
    }

    pub fn remove_account(&mut self, uuid: &str) -> Result<()> {
        self.accounts.retain(|a| a.uuid != uuid);
        if self.config.selected_account.as_deref() == Some(uuid) {
            self.config.selected_account = self.accounts.first().map(|a| a.uuid.clone());
            self.save_config()?;
        }
        self.save_accounts()
    }

    pub fn selected_account(&self) -> Option<&Account> {
        self.config
            .selected_account
            .as_deref()
            .and_then(|u| self.accounts.iter().find(|a| a.uuid == u))
            .or_else(|| self.accounts.first())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Loader;

    const ID: &str = "12345678-1234-1234-1234-123456789abc";

    fn open(root: &Path) -> Store {
        Store::open(Paths::new(root)).unwrap()
    }

    #[test]
    fn stale_content_record_cannot_make_instance_disappear() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let stale = root.join("old-layout").join("old.jar");
        std::fs::write(
            root.join("accounts.json"),
            serde_json::to_string(&json!({
                "accounts": [],
                "profiles": [{
                    "id": ID,
                    "name": "My preserved instance",
                    "mcVersion": "1.21.11",
                    "loader": "fabric",
                    "mods": [{"title": "old record", "filename": "old.jar",
                              "file": stale.to_string_lossy()}],
                    "created": 10
                }]
            }))
            .unwrap(),
        )
        .unwrap();

        let store = open(root);
        assert_eq!(store.profiles.len(), 1);
        assert_eq!(store.profiles[0].id, ID);
        assert_eq!(store.profiles[0].name, "My preserved instance");
        assert_eq!(store.profiles[0].loader, Loader::Fabric);
        assert_eq!(store.profiles[0].created, 10);
        assert!(store.profiles[0].mods.is_empty());
    }

    #[test]
    fn valid_mod_kept_and_garbage_fields_default() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let jar = root.join("instances").join(ID).join("mods").join("a.jar");
        std::fs::write(
            root.join("accounts.json"),
            serde_json::to_string(&json!({"profiles": [{
                "id": ID, "name": 5, "loader": "custom", "color": "red",
                "mods": [{"filename": "a.jar", "file": jar.to_string_lossy(), "x": 1},
                         {"type": "bad"}]
            }]}))
            .unwrap(),
        )
        .unwrap();
        let store = open(root);
        let p = &store.profiles[0];
        assert_eq!(p.name, "New Profile");
        assert_eq!(p.loader, Loader::Vanilla);
        assert_eq!(p.color, None);
        assert_eq!(p.mods.len(), 1);
        assert_eq!(p.mods[0].extra.get("x"), Some(&json!(1)));
    }

    #[test]
    fn empty_store_gets_default_profile_and_persists() {
        let dir = tempfile::tempdir().unwrap();
        let store = open(dir.path());
        assert_eq!(store.profiles.len(), 1);
        assert_eq!(
            store.config.selected_profile.as_deref(),
            Some(store.profiles[0].id.as_str())
        );
        let again = open(dir.path());
        assert_eq!(again.profiles, store.profiles);
        assert!(dir.path().join("config.json").exists());
    }

    #[test]
    fn update_and_delete_profile() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = open(dir.path());
        let id = store.profiles[0].id.clone();
        store
            .update_profile(&id, &json!({"name": " Renamed ", "ram": 4096}))
            .unwrap();
        assert_eq!(store.get_profile(&id).unwrap().name, "Renamed");
        assert!(store.update_profile(&id, &json!({"id": "x"})).is_err());
        let outside = json!({"mods": [{"file": "/etc/passwd"}]});
        assert!(store.update_profile(&id, &outside).is_err());
        store.delete_profile(&id).unwrap();
        assert!(store.get_profile(&id).is_none());
        assert!(store.config.selected_profile.is_none());
        assert!(store.delete_profile("../x").is_err());
    }

    #[test]
    fn sibling_prefix_and_traversal_are_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(dir.path());
        let base = paths.instance_dir(ID).unwrap();
        let mk = |f: PathBuf| ModRecord {
            file: Some(f.to_string_lossy().into_owned()),
            ..Default::default()
        };
        assert!(safe_mod(&paths, ID, mk(base.join("mods").join("a.jar"))).is_ok());
        assert!(safe_mod(&paths, ID, mk(base.join("mods").join("..").join("x"))).is_err());
        let sibling = PathBuf::from(format!("{}-evil", base.display())).join("mods/a.jar");
        assert!(safe_mod(&paths, ID, mk(sibling)).is_err());
        assert!(safe_mod(&paths, ID, mk(base.join("saves").join("a.jar"))).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_mod_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(dir.path());
        let mods = paths.instance_dir(ID).unwrap().join("mods");
        std::fs::create_dir_all(&mods).unwrap();
        let link = mods.join("l.jar");
        std::os::unix::fs::symlink("/etc/passwd", &link).unwrap();
        let m = ModRecord {
            file: Some(link.to_string_lossy().into_owned()),
            ..Default::default()
        };
        assert!(safe_mod(&paths, ID, m).is_err());
    }

    #[test]
    fn corrupt_accounts_file_recovers_from_backup() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = open(dir.path());
        store
            .update_profile(&store.profiles[0].id.clone(), &json!({"name": "A"}))
            .unwrap();
        store
            .update_profile(&store.profiles[0].id.clone(), &json!({"name": "B"}))
            .unwrap();
        std::fs::write(dir.path().join("accounts.json"), "{\"profiles\":").unwrap();
        let again = open(dir.path());
        assert_eq!(again.profiles.len(), 1);
        assert_eq!(again.profiles[0].name, "A");
    }

    #[test]
    fn legacy_profiles_without_group_and_tags_load_and_new_fields_persist() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::write(
            root.join("accounts.json"),
            serde_json::to_string(&json!({"accounts": [], "profiles": [
                {"id": ID, "name": "Old", "mcVersion": "1.20.1", "loader": "vanilla", "created": 5}
            ]}))
            .unwrap(),
        )
        .unwrap();
        let mut store = open(root);
        assert_eq!(store.profiles.len(), 1);
        assert_eq!(store.profiles[0].group, None);
        assert!(store.profiles[0].tags.is_empty());
        store
            .update_profile(
                ID,
                &json!({"group": " Pals ", "tags": ["smp", "SMP", "1.20"]}),
            )
            .unwrap();
        let again = open(root);
        assert_eq!(again.profiles[0].group.as_deref(), Some("Pals"));
        assert_eq!(again.profiles[0].tags, ["smp", "1.20"]);
        // Clearing writes neither field back.
        let mut again = again;
        again
            .update_profile(ID, &json!({"group": null, "tags": []}))
            .unwrap();
        let raw = std::fs::read_to_string(root.join("accounts.json")).unwrap();
        assert!(!raw.contains("\"group\"") && !raw.contains("\"tags\""));
    }
}
