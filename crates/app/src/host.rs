//! Things only the desktop shell can really do, behind traits so the service
//! stays testable and Tauri-free.

use async_trait::async_trait;
use mango_auth::{DeviceKeyStore, Vault};
use std::fmt;
use std::path::Path;
use std::sync::{Arc, Mutex};

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

/// Open an http(s) link in the default browser. Policy (who may be opened)
/// lives in the service; implementations only open.
pub trait ExternalOpener: Send + Sync {
    fn open_url(&self, url: &str) -> Result<(), String>;
}

/// Spawns the OS URL handler; does nothing when disabled (tests, headless).
#[derive(Debug, Clone, Copy)]
pub struct SystemExternal {
    pub enabled: bool,
}

impl ExternalOpener for SystemExternal {
    fn open_url(&self, url: &str) -> Result<(), String> {
        if !self.enabled {
            return Ok(());
        }
        let mut cmd = if cfg!(target_os = "windows") {
            let mut c = std::process::Command::new("rundll32");
            c.arg("url.dll,FileProtocolHandler");
            c
        } else if cfg!(target_os = "macos") {
            std::process::Command::new("open")
        } else {
            std::process::Command::new("xdg-open")
        };
        cmd.arg(url).spawn().map(|_| ()).map_err(|e| e.to_string())
    }
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

/// What the Microsoft sign-in window reported.
#[derive(Clone, PartialEq, Eq)]
pub enum LoginOutcome {
    /// The intercepted redirect: the full URL that starts with the redirect
    /// prefix (a bare authorization code is accepted too). Parsed by the
    /// service, so a denied sign-in arriving as a redirect still means
    /// cancelled.
    Code(String),
    /// The user closed the window.
    Cancelled,
    /// The window could not be shown or navigated somewhere forbidden.
    Error(String),
}

// The redirect carries a one-time authorization code: keep it out of logs.
impl fmt::Debug for LoginOutcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LoginOutcome::Code(_) => f.write_str("Code(<redacted>)"),
            LoginOutcome::Cancelled => f.write_str("Cancelled"),
            LoginOutcome::Error(e) => write!(f, "Error({e:?})"),
        }
    }
}

/// The isolated sign-in window (src-tauri: a non-persistent webview; CLI:
/// print the URL, read the pasted redirect).
#[async_trait]
pub trait LoginWindow: Send + Sync {
    /// Open `authorize_url`, wait until a navigation reaches a URL starting
    /// with `redirect_prefix` (return it) or the user gives up.
    async fn authorize(&self, authorize_url: &str, redirect_prefix: &str) -> LoginOutcome;
}

/// Hosts the sign-in window may load (each also matches its subdomains).
const LOGIN_HOSTS: [&str; 7] = [
    "live.com",
    "microsoft.com",
    "microsoftonline.com",
    "xboxlive.com",
    "msauth.net",
    "msftauth.net",
    "msftauthimages.net",
];

/// Navigation allow-list of the sign-in window: https only, no credentials,
/// Microsoft/Xbox/Live hosts and the msauth/msftauth asset CDNs.
pub fn login_navigation_allowed(raw: &str) -> bool {
    let Ok(url) = url::Url::parse(raw) else {
        return false;
    };
    if url.scheme() != "https" || !url.username().is_empty() || url.password().is_some() {
        return false;
    }
    let Some(host) = url.host_str() else {
        return false;
    };
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    LOGIN_HOSTS
        .iter()
        .any(|d| host == *d || host.ends_with(&format!(".{d}")))
}

/// True for the redirect the window must intercept (and never load).
pub fn is_login_redirect(raw: &str, redirect_prefix: &str) -> bool {
    raw.starts_with(redirect_prefix)
}

/// Refuses to sign in; the default until the shell provides a window.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoLoginWindow;

#[async_trait]
impl LoginWindow for NoLoginWindow {
    async fn authorize(&self, _authorize_url: &str, _redirect_prefix: &str) -> LoginOutcome {
        LoginOutcome::Error("no sign-in window available".into())
    }
}

/// Feeds a canned outcome and records what it was asked to open (tests).
#[derive(Debug)]
pub struct CannedLoginWindow {
    outcome: LoginOutcome,
    requests: Mutex<Vec<(String, String)>>,
}

impl CannedLoginWindow {
    pub fn new(outcome: LoginOutcome) -> Self {
        Self {
            outcome,
            requests: Mutex::new(Vec::new()),
        }
    }

    /// `(authorize_url, redirect_prefix)` of every call so far.
    pub fn requests(&self) -> Vec<(String, String)> {
        self.requests
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }
}

#[async_trait]
impl LoginWindow for CannedLoginWindow {
    async fn authorize(&self, authorize_url: &str, redirect_prefix: &str) -> LoginOutcome {
        self.requests
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push((authorize_url.to_string(), redirect_prefix.to_string()));
        self.outcome.clone()
    }
}

/// Native file and folder pickers (src-tauri: the dialog plugin).
#[async_trait]
pub trait FileDialog: Send + Sync {
    /// One image file, `None` when cancelled.
    async fn pick_image(&self, title: Option<String>) -> Option<std::path::PathBuf>;
    /// One folder, `None` when cancelled.
    async fn pick_folder(&self, title: Option<String>) -> Option<std::path::PathBuf>;
}

/// Always cancelled; the default until the shell provides dialogs.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoFileDialog;

#[async_trait]
impl FileDialog for NoFileDialog {
    async fn pick_image(&self, _title: Option<String>) -> Option<std::path::PathBuf> {
        None
    }
    async fn pick_folder(&self, _title: Option<String>) -> Option<std::path::PathBuf> {
        None
    }
}

/// Answers with fixed paths and records the titles it was asked with (tests).
#[derive(Debug, Default)]
pub struct CannedFileDialog {
    pub image: Option<std::path::PathBuf>,
    pub folder: Option<std::path::PathBuf>,
    titles: Mutex<Vec<(&'static str, Option<String>)>>,
}

impl CannedFileDialog {
    pub fn new(image: Option<std::path::PathBuf>, folder: Option<std::path::PathBuf>) -> Self {
        Self {
            image,
            folder,
            titles: Mutex::new(Vec::new()),
        }
    }

    /// `("image" | "folder", title)` of every call so far.
    pub fn calls(&self) -> Vec<(&'static str, Option<String>)> {
        self.titles
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }
}

#[async_trait]
impl FileDialog for CannedFileDialog {
    async fn pick_image(&self, title: Option<String>) -> Option<std::path::PathBuf> {
        self.titles
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(("image", title));
        self.image.clone()
    }
    async fn pick_folder(&self, title: Option<String>) -> Option<std::path::PathBuf> {
        self.titles
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(("folder", title));
        self.folder.clone()
    }
}

/// The shell-provided capabilities.
#[derive(Clone)]
pub struct Host {
    pub opener: Arc<dyn OpenFolder>,
    pub clipboard: Arc<dyn ClipboardImage>,
    pub login: Arc<dyn LoginWindow>,
    pub external: Arc<dyn ExternalOpener>,
    pub dialog: Arc<dyn FileDialog>,
    /// Encrypts account secrets with the per-device key.
    pub vault: Arc<Vault>,
    /// Installs a verified update (src-tauri: the platform installer).
    pub installer: Arc<dyn mango_updater::Installer>,
}

#[cfg(not(test))]
fn default_vault() -> Vault {
    Vault::system()
}

/// Unit tests of this crate never touch the real OS keychain.
#[cfg(test)]
fn default_vault() -> Vault {
    Vault::new(Arc::new(mango_auth::MemoryDeviceKey::new()), None)
}

impl Host {
    /// OS file manager, no clipboard.
    pub fn system(open_enabled: bool) -> Host {
        Host {
            opener: Arc::new(SystemOpener {
                enabled: open_enabled,
            }),
            clipboard: Arc::new(NoClipboard),
            login: Arc::new(NoLoginWindow),
            external: Arc::new(SystemExternal {
                enabled: open_enabled,
            }),
            dialog: Arc::new(NoFileDialog),
            vault: Arc::new(default_vault()),
            installer: Arc::new(mango_updater::NoopInstaller),
        }
    }

    /// Replace the update installer (default: does nothing).
    pub fn with_installer(mut self, installer: Arc<dyn mango_updater::Installer>) -> Host {
        self.installer = installer;
        self
    }

    pub fn with_external(mut self, external: Arc<dyn ExternalOpener>) -> Host {
        self.external = external;
        self
    }

    pub fn with_dialog(mut self, dialog: Arc<dyn FileDialog>) -> Host {
        self.dialog = dialog;
        self
    }

    pub fn with_login(mut self, login: Arc<dyn LoginWindow>) -> Host {
        self.login = login;
        self
    }

    /// Replace the vault (tests: in-memory device key).
    pub fn with_vault(mut self, vault: Arc<Vault>) -> Host {
        self.vault = vault;
        self
    }

    /// Vault over an arbitrary device key store, no legacy keyring.
    pub fn with_device_keys(self, keys: Arc<dyn DeviceKeyStore>) -> Host {
        self.with_vault(Arc::new(Vault::new(keys, None)))
    }

    pub fn with_clipboard(mut self, clipboard: Arc<dyn ClipboardImage>) -> Host {
        self.clipboard = clipboard;
        self
    }
}
