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
            .filter(|bundle| mac_bundle_replaceable(bundle))
            .map(Target::MacApp),
        _ => None,
    }
}

/// A macOS bundle can replace itself only from a writable folder it really lives in: not from the read-only copy macOS
/// runs a quarantined app from (App Translocation, e.g. straight out of Downloads), not from a mounted DMG, and not where
/// the user may not write (/Applications for a non-admin). There the app says it cannot update itself.
fn mac_bundle_replaceable(bundle: &Path) -> bool {
    let s = bundle.to_string_lossy();
    if s.contains("/AppTranslocation/") || s.starts_with("/Volumes/") {
        return false;
    }
    bundle.parent().is_some_and(|dir| {
        let probe = dir.join(format!(".mangoclient-write-test-{}", std::process::id()));
        let ok = std::fs::write(&probe, b"").is_ok();
        let _ = std::fs::remove_file(&probe);
        ok
    })
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
