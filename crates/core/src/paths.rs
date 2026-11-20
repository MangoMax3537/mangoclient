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

/// The folder a browser saves downloads to, per platform: the XDG user dir
/// (`user-dirs.dirs`) on Linux, `~/Downloads` on macOS, `%USERPROFILE%\\Downloads`
/// on Windows. `None` when no home directory is known.
pub fn default_downloads_dir() -> Option<PathBuf> {
    let var = |k: &str| std::env::var_os(k).filter(|v| !v.is_empty());
    let home = if cfg!(windows) {
        var("USERPROFILE").or_else(|| var("HOME"))
    } else {
        var("HOME")
    }
    .map(PathBuf::from)?;
    Some(downloads_dir_in(
        &home,
        var("XDG_CONFIG_HOME").map(PathBuf::from).as_deref(),
        var("XDG_DOWNLOAD_DIR")
            .and_then(|v| v.into_string().ok())
            .as_deref(),
        cfg!(all(unix, not(target_os = "macos"))),
    ))
}

/// [`default_downloads_dir`] with every input explicit (for tests).
pub fn downloads_dir_in(
    home: &Path,
    xdg_config_home: Option<&Path>,
    xdg_download_env: Option<&str>,
    xdg: bool,
) -> PathBuf {
    let fallback = home.join("Downloads");
    if !xdg {
        return fallback;
    }
    let expand = |raw: &str| -> Option<PathBuf> {
        let raw = raw.trim().trim_matches('"');
        let path = match raw.strip_prefix("$HOME") {
            Some(rest) => format!("{}{rest}", home.display()),
            None => raw.to_string(),
        };
        let p = PathBuf::from(path);
        (p.is_absolute() && !p.as_os_str().to_string_lossy().contains('\0')).then_some(p)
    };
    if let Some(p) = xdg_download_env.and_then(expand) {
        return p;
    }
    let config = xdg_config_home
        .map(Path::to_path_buf)
        .filter(|c| c.is_absolute())
        .unwrap_or_else(|| home.join(".config"));
    std::fs::read_to_string(config.join("user-dirs.dirs"))
        .ok()
        .and_then(|text| {
            text.lines().find_map(|line| {
                line.trim()
                    .strip_prefix("XDG_DOWNLOAD_DIR=")
                    .and_then(expand)
            })
        })
        .unwrap_or(fallback)
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

    /// The profile's background picture, `covers/<uuid>-bg.png`.
    pub fn background_file(&self, profile_id: &str) -> Result<PathBuf> {
        Ok(self
            .covers()
            .join(format!("{}-bg.png", validate_profile_id(profile_id)?)))
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
    fn downloads_dir_follows_xdg_user_dirs_and_falls_back() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();
        assert_eq!(
            downloads_dir_in(home, None, None, true),
            home.join("Downloads")
        );
        std::fs::create_dir_all(home.join(".config")).unwrap();
        std::fs::write(
            home.join(".config/user-dirs.dirs"),
            "# c\nXDG_DESKTOP_DIR=\"$HOME/Desk\"\nXDG_DOWNLOAD_DIR=\"$HOME/Heruntergeladen\"\n",
        )
        .unwrap();
        assert_eq!(
            downloads_dir_in(home, None, None, true),
            home.join("Heruntergeladen")
        );
        // Relative values are ignored; the env var wins when absolute.
        std::fs::write(
            home.join(".config/user-dirs.dirs"),
            "XDG_DOWNLOAD_DIR=\"rel\"\n",
        )
        .unwrap();
        assert_eq!(
            downloads_dir_in(home, None, None, true),
            home.join("Downloads")
        );
        assert_eq!(
            downloads_dir_in(home, None, Some("/data/dl"), true),
            PathBuf::from("/data/dl")
        );
        // Other platforms never read user-dirs.dirs.
        assert_eq!(
            downloads_dir_in(home, None, Some("/data/dl"), false),
            home.join("Downloads")
        );
    }

    #[test]
    fn layout() {
        let p = Paths::new("/r");
        assert_eq!(p.asset_indexes(), PathBuf::from("/r/assets/indexes"));
        assert_eq!(p.accounts_file(), PathBuf::from("/r/accounts.json"));
    }
}
