//! Java discovery, probing, Temurin download and runtime resolution.
//!
//! Port of `src/main/java.js`. No Tauri dependency.

mod adoptium;
mod discover;
mod error;
mod platform;
mod probe;
mod resolve;

pub use adoptium::{download_runtime, DEFAULT_ADOPTIUM_BASE};
pub use discover::{
    find_system_javas, java_executable, launch_executable, list_managed_runtimes, SearchRoots,
};
pub use error::JavaError;
pub use mango_net::ProgressFn;
pub use platform::{Arch, Os, Platform};
pub use probe::{parse_java_version, probe, JavaInfo};
pub use resolve::{
    decide, required_major, required_major_from_json, resolve_java, Candidate, CustomJava,
    Decision, JavaSource, ResolveOptions, Resolved,
};
