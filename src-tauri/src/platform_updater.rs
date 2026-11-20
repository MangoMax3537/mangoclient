//! Runtime wiring of the platform installers from `mango-updater`.
//!
//! All logic (path checks, backup, rollback) lives in `crates/updater`; this
//! only picks the `Target` for the running install and supplies the quit
//! callback. The result goes into `Host::with_installer`; the service hands it
//! to the updater (the "Restart now" button and the launch-time update both
//! end in `Installer::apply_on_quit`, which relaunches and exits the app).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use mango_updater::{PlatformInstaller, ProcessSpawner, Target};
use tauri::AppHandle;

/// Which kind of install is running, or `None` when it cannot replace itself
/// (dev run, tar.gz or distro-package Linux install).
pub fn detect_target() -> Option<Target> {
    match std::env::consts::OS {
        "windows" => Some(Target::WindowsNsis),
        "linux" => std::env::var_os("APPIMAGE")
            .filter(|v| !v.is_empty())
            .map(|v| Target::AppImage(PathBuf::from(v))),
        "macos" => std::env::current_exe()
            .ok()
            .and_then(|exe| {
                exe.ancestors()
                    .find(|p| p.extension().is_some_and(|e| e == "app"))
                    .map(Path::to_path_buf)
            })
            .map(Target::MacApp),
        _ => None,
    }
}

/// The installer for this install; `download_dir` is the updater's download
/// directory, the only place a verified file may come from.
pub fn build(app: AppHandle, download_dir: &Path) -> Option<Arc<PlatformInstaller>> {
    let target = detect_target()?;
    let installer = PlatformInstaller::new(
        target,
        download_dir,
        Arc::new(ProcessSpawner),
        Arc::new(move || app.exit(0)),
    );
    // A backup from the previous update is obsolete once the new build runs.
    installer.cleanup_backup();
    Some(Arc::new(installer))
}
