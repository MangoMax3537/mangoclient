//! Minecraft version resolution, installation and launch command building.
//!
//! Port of `src/main/installer.js` and the argument-building part of
//! `src/main/launcher.js`. Spawning, log tailing and session adoption live in
//! a later crate. No Tauri dependency.

mod error;
mod host;
mod install;
mod launch;
mod libraries;
mod natives;
mod rules;
mod version;

#[cfg(test)]
mod tests;

pub use error::{MinecraftError, Result};
pub use host::{HostArch, HostEnv, HostOs};
pub use install::{
    install_version, Endpoints, InstallOptions, InstallProgress, InstallResult, LogCb, Phase,
    ProgressCb,
};
pub use launch::{
    build_launch_command, expand_args, substitute, LaunchAccount, LaunchCommand, LaunchInput,
    Preset,
};
pub use libraries::{collect_libraries, is_safe_relative, maven_to_path, Collected, LibraryFile};
pub use natives::{extract_natives, STAMP_FILE};
pub use rules::{rules_allow, Features};
pub use version::{
    get_version_manifest, library_key, merge_version_json, resolve_version, validate_safe_name,
    ResolvedVersion, MANIFEST_TTL, RESOURCES_URL, VERSION_MANIFEST_URL,
};
