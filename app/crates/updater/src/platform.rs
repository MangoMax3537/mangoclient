/// Injectable platform description (so tests can pretend to be anything).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Platform {
    /// `std::env::consts::OS` style: `windows`, `macos`, `linux`.
    pub os: String,
    /// `std::env::consts::ARCH` style: `x86_64`, `aarch64`.
    pub arch: String,
    /// Running as an AppImage (the `APPIMAGE` env var is set).
    pub appimage: bool,
}

impl Platform {
    pub fn detect() -> Self {
        Self {
            os: std::env::consts::OS.into(),
            arch: std::env::consts::ARCH.into(),
            appimage: std::env::var_os("APPIMAGE").is_some_and(|v| !v.is_empty()),
        }
    }

    /// Whether this install can replace itself. A tar.gz Linux install cannot.
    pub fn supported(&self) -> bool {
        match self.os.as_str() {
            "windows" | "macos" => true,
            "linux" => self.appimage,
            _ => false,
        }
    }

    /// Manifest keys to try, most specific first.
    pub fn manifest_keys(&self) -> Vec<String> {
        let os = if self.os == "macos" {
            "darwin"
        } else {
            &self.os
        };
        let base = format!("{os}-{}", self.arch);
        if self.os == "linux" && self.appimage {
            vec![format!("{base}-appimage"), base]
        } else {
            vec![base]
        }
    }
}
