//! Modrinth API client, content install/uninstall/toggle/update logic and
//! `.mrpack` installation. Port of `src/main/modrinth.js`.
//!
//! Nothing here writes `accounts.json`: every function takes explicit paths
//! or records and returns the updated mod list for the caller to persist.

mod client;
mod deps;
mod error;
mod install;
mod mrpack;
mod types;
mod updates;

#[cfg(test)]
mod tests;

pub use client::{pick_version, primary_file, Client, API, CATEGORY_TTL, METADATA_TTL, SEARCH_TTL};
pub use deps::{
    has_exact_installed_version, hydrate_dependency_metadata, installed_dependents,
    set_enabled_with_dependencies, set_mod_enabled, toggle, uninstall, Affected, Dependent,
    Hydrated, ToggleOutcome, UninstallOutcome,
};
pub use error::{Error, Result};
pub use install::{
    install_project, loader_name, merge_installed, mod_type_of, mods_dir, target_dir_for, LogFn,
};
pub use mrpack::{
    detect_loader, inspect_pack, install_modpack, ModpackInfo, ModpackProgress, PackSummary,
    ProgressFn, MAX_INDEX_BYTES,
};
pub use types::{Dependency, Hashes, Project, SearchParams, Version, VersionFile};
pub use updates::{check_all_updates, check_updates, ProfileUpdates, RequiredBy, UpdateInfo};
