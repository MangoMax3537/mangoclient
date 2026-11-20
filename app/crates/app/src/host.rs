//! Things only the desktop shell can really do, behind traits so the service
//! stays testable and Tauri-free.

use std::path::Path;
use std::sync::Arc;

/// Open a folder or reveal a file in the OS file manager.
pub trait OpenFolder: Send + Sync {
    fn open(&self, dir: &Path) -> std::io::Result<()>;
    /// Show `file` selected in its folder.
    fn reveal(&self, file: &Path) -> std::io::Result<()>;
}

/// Put an image file on the system clipboard (the picture, not its path).
pub trait ClipboardImage: Send + Sync {
    fn copy_image(&self, file: &Path) -> Result<(), String>;
}

/// Spawns `xdg-open` / `open` / `explorer`. With `enabled == false` it does
/// nothing (tests, headless runs).
#[derive(Debug, Clone, Copy)]
pub struct SystemOpener {
    pub enabled: bool,
}

impl OpenFolder for SystemOpener {
    fn open(&self, dir: &Path) -> std::io::Result<()> {
        if !self.enabled {
            return Ok(());
        }
        std::process::Command::new(file_manager())
            .arg(dir)
            .spawn()
            .map(|_| ())
    }

    fn reveal(&self, file: &Path) -> std::io::Result<()> {
        if !self.enabled {
            return Ok(());
        }
        let mut cmd = std::process::Command::new(file_manager());
        if cfg!(target_os = "windows") {
            let mut arg = std::ffi::OsString::from("/select,");
            arg.push(file);
            cmd.arg(arg);
        } else if cfg!(target_os = "macos") {
            cmd.arg("-R").arg(file);
        } else {
            // No portable "select" on Linux: open the containing folder.
            cmd.arg(file.parent().unwrap_or(file));
        }
        cmd.spawn().map(|_| ())
    }
}

fn file_manager() -> &'static str {
    if cfg!(target_os = "windows") {
        "explorer"
    } else if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    }
}

/// Fails every copy; the default until the shell provides a clipboard.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoClipboard;

impl ClipboardImage for NoClipboard {
    fn copy_image(&self, _file: &Path) -> Result<(), String> {
        Err("no clipboard available".into())
    }
}

/// The shell-provided capabilities.
#[derive(Clone)]
pub struct Host {
    pub opener: Arc<dyn OpenFolder>,
    pub clipboard: Arc<dyn ClipboardImage>,
}

impl Host {
    /// OS file manager, no clipboard.
    pub fn system(open_enabled: bool) -> Host {
        Host {
            opener: Arc::new(SystemOpener {
                enabled: open_enabled,
            }),
            clipboard: Arc::new(NoClipboard),
        }
    }

    pub fn with_clipboard(mut self, clipboard: Arc<dyn ClipboardImage>) -> Host {
        self.clipboard = clipboard;
        self
    }
}
