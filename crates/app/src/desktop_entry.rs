//! Linux AppImage desktop integration: a menu entry and icons in the user's applications.
//!
//! An AppImage is one file that registers nowhere, so it is missing from the application menu and, on Wayland, from the
//! task bar (the compositor finds a window's icon only through a desktop entry named like its app id). With the
//! `desktopIntegration` setting (on by default, a box in the first-run setup) the launcher keeps
//! `$XDG_DATA_HOME/applications/dev.mangomax.mangoclient.desktop` plus hicolor icons. The entry's `Exec` follows the
//! AppImage, whose file name changes with updates, so it is rewritten whenever it differs. Only files carrying our
//! marker line are ever replaced or removed: a hand-made or packaged entry is left alone.
//!
//! Nothing happens outside an AppImage (`$APPIMAGE` unset): the .deb, Windows and macOS installers register the app.

use std::path::{Path, PathBuf};

/// App id of the window (Tauri `identifier`) = desktop file name = icon name.
pub(crate) const APP_ID: &str = "dev.mangomax.mangoclient";
const MARKER: &str = "X-MangoClient-Integration=1";

const ICONS: [(u32, &[u8]); 6] = [
    (32, include_bytes!("../../../assets/brand/icons/32x32.png")),
    (48, include_bytes!("../../../assets/brand/icons/48x48.png")),
    (64, include_bytes!("../../../assets/brand/icons/64x64.png")),
    (
        128,
        include_bytes!("../../../assets/brand/icons/128x128.png"),
    ),
    (
        256,
        include_bytes!("../../../assets/brand/icons/256x256.png"),
    ),
    (
        512,
        include_bytes!("../../../assets/brand/icons/512x512.png"),
    ),
];

/// What the setting acts on: the running AppImage and the user's data dir.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DesktopIntegration {
    appimage: PathBuf,
    data_home: PathBuf,
}

impl DesktopIntegration {
    /// The running AppImage on Linux (`$APPIMAGE`, absolute, an existing file), else `None`.
    pub(crate) fn detect() -> Option<Self> {
        if !cfg!(target_os = "linux") {
            return None;
        }
        let appimage = PathBuf::from(std::env::var_os("APPIMAGE").filter(|v| !v.is_empty())?);
        if !appimage.is_absolute() || !appimage.is_file() {
            return None;
        }
        Some(Self::new(appimage, mango_core::Paths::xdg_data_home()?))
    }

    pub(crate) fn new(appimage: PathBuf, data_home: PathBuf) -> Self {
        Self {
            appimage,
            data_home,
        }
    }

    pub(crate) fn desktop_file(&self) -> PathBuf {
        self.data_home
            .join("applications")
            .join(format!("{APP_ID}.desktop"))
    }

    fn icon_file(&self, size: u32) -> PathBuf {
        self.data_home
            .join("icons/hicolor")
            .join(format!("{size}x{size}"))
            .join("apps")
            .join(format!("{APP_ID}.png"))
    }

    /// The entry for the current AppImage path.
    pub(crate) fn render(&self) -> String {
        let path = self.appimage.to_string_lossy();
        format!(
            "[Desktop Entry]\nType=Application\nName=Mango Client\nGenericName=Minecraft Launcher\n\
             Comment=Minecraft launcher with profiles, Modrinth and Microsoft accounts\n\
             Exec={} %U\nTryExec={}\nIcon={APP_ID}\nTerminal=false\nCategories=Game;\n\
             Keywords=minecraft;launcher;mango;\nStartupWMClass={APP_ID}\n{MARKER}\n",
            exec_quote(&path),
            escape_value(&path),
        )
    }

    /// Writes the entry and icons when missing or outdated. `Ok(false)` = nothing to do, or a foreign entry is in the way.
    pub(crate) fn install(&self) -> std::io::Result<bool> {
        let file = self.desktop_file();
        let want = self.render();
        let mut changed = false;
        match std::fs::read_to_string(&file) {
            Ok(have) if have == want => {}
            Ok(have) if !have.contains(MARKER) => return Ok(false),
            _ => {
                write_atomic(&file, want.as_bytes())?;
                changed = true;
            }
        }
        for (size, bytes) in ICONS {
            let icon = self.icon_file(size);
            if std::fs::read(&icon).ok().as_deref() != Some(bytes) {
                write_atomic(&icon, bytes)?;
                changed = true;
            }
        }
        if changed {
            refresh_menus(&self.data_home);
        }
        Ok(changed)
    }

    /// Removes our entry and icons (a foreign entry is kept). `Ok(true)` when something was removed.
    pub(crate) fn remove(&self) -> std::io::Result<bool> {
        let file = self.desktop_file();
        let ours = std::fs::read_to_string(&file).is_ok_and(|t| t.contains(MARKER));
        if !ours {
            return Ok(false);
        }
        std::fs::remove_file(&file)?;
        for (size, _) in ICONS {
            let _ = std::fs::remove_file(self.icon_file(size));
        }
        refresh_menus(&self.data_home);
        Ok(true)
    }
}

/// Desktop Entry Spec: a quoted Exec argument escapes `"`, `` ` ``, `$` and `\` with a backslash, and the value itself
/// escapes `\` once more.
fn exec_quote(path: &str) -> String {
    let mut q = String::from("\"");
    for c in path.chars() {
        if matches!(c, '"' | '`' | '$' | '\\') {
            q.push('\\');
        }
        q.push(c);
    }
    q.push('"');
    escape_value(&q)
}

fn escape_value(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('\n', "\\n")
        .replace('\t', "\\t")
        .replace('\r', "\\r")
}

fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let dir = path
        .parent()
        .ok_or_else(|| std::io::Error::other("no parent directory"))?;
    std::fs::create_dir_all(dir)?;
    let tmp = dir.join(format!(
        ".{}.tmp",
        path.file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default()
    ));
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)
}

/// Best effort, never blocking: menus and icon caches that do not watch the folders pick the change up here.
fn refresh_menus(data_home: &Path) {
    let apps = data_home.join("applications");
    for (cmd, args) in [
        ("update-desktop-database", vec![apps.as_os_str().to_owned()]),
        ("kbuildsycoca6", vec![]),
        ("kbuildsycoca5", vec![]),
    ] {
        let _ = std::process::Command::new(cmd)
            .args(args)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn();
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn fixture(appimage: &str) -> (tempfile::TempDir, DesktopIntegration) {
        let tmp = tempfile::tempdir().unwrap();
        let d = DesktopIntegration::new(PathBuf::from(appimage), tmp.path().join("share"));
        (tmp, d)
    }

    #[test]
    fn installs_entry_and_icons_and_follows_the_appimage_path() {
        let (tmp, d) = fixture("/home/a b/Apps/MangoClient.AppImage");
        assert!(d.install().unwrap());
        let text = std::fs::read_to_string(d.desktop_file()).unwrap();
        assert!(
            text.contains("Exec=\"/home/a b/Apps/MangoClient.AppImage\" %U\n"),
            "{text}"
        );
        assert!(text.contains("Icon=dev.mangomax.mangoclient\n"));
        assert!(text.contains("StartupWMClass=dev.mangomax.mangoclient\n"));
        assert!(text.contains(MARKER));
        for (size, bytes) in ICONS {
            assert_eq!(std::fs::read(d.icon_file(size)).unwrap(), bytes);
        }
        // unchanged: nothing to write
        assert!(!d.install().unwrap());
        // an update renamed the AppImage: the entry follows
        let moved = DesktopIntegration::new(
            PathBuf::from("/opt/MangoClient_2.0.1_amd64.AppImage"),
            tmp.path().join("share"),
        );
        assert!(moved.install().unwrap());
        assert!(std::fs::read_to_string(moved.desktop_file())
            .unwrap()
            .contains("Exec=\"/opt/MangoClient_2.0.1_amd64.AppImage\" %U"));
    }

    #[test]
    fn a_foreign_entry_is_never_replaced_or_removed() {
        let (_tmp, d) = fixture("/x/MangoClient.AppImage");
        std::fs::create_dir_all(d.desktop_file().parent().unwrap()).unwrap();
        std::fs::write(d.desktop_file(), "[Desktop Entry]\nName=Mine\n").unwrap();
        assert!(!d.install().unwrap());
        assert!(!d.remove().unwrap());
        assert_eq!(
            std::fs::read_to_string(d.desktop_file()).unwrap(),
            "[Desktop Entry]\nName=Mine\n"
        );
    }

    #[test]
    fn remove_deletes_our_entry_and_icons() {
        let (_tmp, d) = fixture("/x/MangoClient.AppImage");
        d.install().unwrap();
        assert!(d.remove().unwrap());
        assert!(!d.desktop_file().exists());
        assert!(!d.icon_file(256).exists());
        assert!(!d.remove().unwrap());
    }

    #[test]
    fn exec_quoting_follows_the_desktop_entry_spec() {
        // `$` and `"` are escaped inside the quotes, and every backslash is doubled once more for the value
        assert_eq!(exec_quote("/a/$x\"y"), "\"/a/\\\\$x\\\\\"y\"");
        assert_eq!(exec_quote("/plain/path"), "\"/plain/path\"");
    }
}
