//! Every on-disk location of the launcher. Layout matches the 1.3.4 Electron app.

use std::path::{Path, PathBuf};

use crate::error::{Error, Result};

/// Canonical 8-4-4-4-12 hex UUID (case-insensitive).
pub fn is_canonical_uuid(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 36
        && b.iter().enumerate().all(|(i, c)| match i {
            8 | 13 | 18 | 23 => *c == b'-',
            _ => c.is_ascii_hexdigit(),
        })
}

pub fn validate_profile_id(value: &str) -> Result<&str> {
    if is_canonical_uuid(value) {
        Ok(value)
    } else {
        Err(Error::InvalidProfileId)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Paths {
    root: PathBuf,
}

impl Paths {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Paths { root: root.into() }
    }

    /// `MANGOCLIENT_HOME`, else the platform default.
    pub fn from_env() -> Self {
        Paths::new(Self::default_root())
    }

    pub fn default_root() -> PathBuf {
        if let Some(home) = std::env::var_os("MANGOCLIENT_HOME") {
            if !home.is_empty() {
                return PathBuf::from(home);
            }
        }
        let home = || {
            std::env::var_os("HOME")
                .or_else(|| std::env::var_os("USERPROFILE"))
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("."))
        };
        if cfg!(windows) {
            let appdata = std::env::var_os("APPDATA")
                .filter(|v| !v.is_empty())
                .map(PathBuf::from)
                .unwrap_or_else(|| home().join("AppData").join("Roaming"));
            appdata.join(".mangoclient")
        } else if cfg!(target_os = "macos") {
            home()
                .join("Library")
                .join("Application Support")
                .join("mangoclient")
        } else {
            home().join(".mangoclient")
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }
    pub fn versions(&self) -> PathBuf {
        self.root.join("versions")
    }
    pub fn libraries(&self) -> PathBuf {
        self.root.join("libraries")
    }
    pub fn assets(&self) -> PathBuf {
        self.root.join("assets")
    }
    pub fn asset_indexes(&self) -> PathBuf {
        self.assets().join("indexes")
    }
    pub fn asset_objects(&self) -> PathBuf {
        self.assets().join("objects")
    }
    pub fn asset_log_configs(&self) -> PathBuf {
        self.assets().join("log_configs")
    }
    pub fn asset_virtual(&self) -> PathBuf {
        self.assets().join("virtual")
    }
    pub fn asset_resources(&self) -> PathBuf {
        self.assets().join("resources")
    }
    pub fn natives(&self) -> PathBuf {
        self.root.join("natives")
    }
    pub fn runtimes(&self) -> PathBuf {
        self.root.join("runtimes")
    }
    pub fn instances(&self) -> PathBuf {
        self.root.join("instances")
    }
    pub fn cache(&self) -> PathBuf {
        self.root.join("cache")
    }
    pub fn covers(&self) -> PathBuf {
        self.root.join("covers")
    }
    pub fn logs(&self) -> PathBuf {
        self.root.join("logs")
    }
    pub fn config_file(&self) -> PathBuf {
        self.root.join("config.json")
    }
    /// Holds both `accounts` and `profiles`.
    pub fn accounts_file(&self) -> PathBuf {
        self.root.join("accounts.json")
    }
    pub fn install_file(&self) -> PathBuf {
        self.root.join("install.json")
    }
    pub fn stats_file(&self) -> PathBuf {
        self.root.join("stats.json")
    }
    pub fn server_icons_file(&self) -> PathBuf {
        self.root.join("server-icons.json")
    }

    pub fn ensure_dirs(&self) -> Result<()> {
        for dir in [
            self.root.clone(),
            self.versions(),
            self.libraries(),
            self.assets(),
            self.asset_indexes(),
            self.asset_objects(),
            self.asset_log_configs(),
            self.asset_virtual(),
            self.asset_resources(),
            self.natives(),
            self.runtimes(),
            self.instances(),
            self.cache(),
            self.covers(),
            self.logs(),
        ] {
            std::fs::create_dir_all(&dir).map_err(|e| Error::io(&dir, e))?;
        }
        Ok(())
    }

    /// A profile's own saves/mods/options directory.
    pub fn instance_dir(&self, profile_id: &str) -> Result<PathBuf> {
        Ok(self.instances().join(validate_profile_id(profile_id)?))
    }

    /// The profile's user-chosen picture, `covers/<uuid>.png`.
    pub fn cover_file(&self, profile_id: &str) -> Result<PathBuf> {
        Ok(self
            .covers()
            .join(format!("{}.png", validate_profile_id(profile_id)?)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_ids_must_be_canonical_uuids() {
        let p = Paths::new("/r");
        assert!(p
            .instance_dir("12345678-1234-1234-1234-123456789abc")
            .is_ok());
        assert!(p.instance_dir("../etc").is_err());
        assert!(p.cover_file("12345678123412341234123456789abc").is_err());
        assert!(p.instance_dir("").is_err());
    }

    #[test]
    fn layout() {
        let p = Paths::new("/r");
        assert_eq!(p.asset_indexes(), PathBuf::from("/r/assets/indexes"));
        assert_eq!(p.accounts_file(), PathBuf::from("/r/accounts.json"));
    }
}
