//! mango-content: local mod reconciliation and the managed performance mods.
//!
//! Port of `src/main/localmods.js` and `src/main/performancemods.js`.

mod error;
mod local;
mod meta;
mod perf;
#[cfg(test)]
mod tests;

pub use error::ContentError;
pub use local::{
    is_own_mango_jar, jar_name, mods_dir, scan_mods_dir, set_local_mod_enabled, sync_profile_mods,
    JarFile, SyncResult,
};
pub use meta::{read_jar_meta, JarMeta};
pub use perf::{
    ensure_performance_mods, is_managed, managed_names, ModrinthLookup, PerfOptions, PerfOutcome,
    PerfState, ResolvedFile, PERFORMANCE_MODS,
};
