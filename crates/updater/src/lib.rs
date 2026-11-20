//! mango-updater: signed self-update logic, independent of Tauri.
//!
//! Flow: fetch `latest.json` -> validate -> newer? -> download with SHA-256 and
//! size check -> verify detached Ed25519 signature -> `Installer::stage`.
//! An unsigned or badly signed file is deleted and never staged.

mod error;
mod installer;
mod manifest;
mod platform;
mod state;
mod updater;
mod verify;

pub use error::{ErrorCode, Result, UpdateError};
pub use installer::{Installer, NoopInstaller};
pub use manifest::{
    PlatformEntry, UpdateManifest, UrlPolicy, DEFAULT_ALLOWED_HOSTS, MAX_UPDATE_SIZE,
    MIN_UPDATE_SIZE,
};
pub use platform::Platform;
pub use state::{DisabledReason, UpdateState, UpdateStatus};
pub use updater::{is_newer, Updater, UpdaterConfig, RECHECK_INTERVAL};
pub use verify::{verify_bytes, verify_file_or_delete, UPDATE_PUBLIC_KEY_B64};

#[cfg(test)]
mod tests;
