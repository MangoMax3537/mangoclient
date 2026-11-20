//! Platform install boundary. No platform code lives in this crate; the shell
//! provides an `Installer`. Expectations per platform:
//!
//! - Windows: `stage` keeps the verified NSIS installer; `apply_on_quit`
//!   spawns it detached with silent flags (`/S`, same appId) once the app exits.
//! - macOS: the update is an archive of the `.app`; `stage` unpacks it next
//!   to the current bundle (via the `archive` crate), `apply_on_quit` swaps
//!   the bundle atomically and relaunches.
//! - Linux: AppImage only. `stage` marks the new AppImage executable;
//!   `apply_on_quit` renames it over `$APPIMAGE`.

use crate::error::Result;
use std::path::Path;

pub trait Installer: Send + Sync {
    /// Prepare a file that is already downloaded and signature-verified.
    fn stage(&self, file: &Path) -> Result<()>;
    /// Arrange for the staged update to be installed when the app quits
    /// (or immediately, then relaunch, when called for a relaunch).
    fn apply_on_quit(&self) -> Result<()>;
}

/// Does nothing; for tests and headless use.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoopInstaller;

impl Installer for NoopInstaller {
    fn stage(&self, _file: &Path) -> Result<()> {
        Ok(())
    }
    fn apply_on_quit(&self) -> Result<()> {
        Ok(())
    }
}
