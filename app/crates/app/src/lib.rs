//! mango-app: Tauri-free service layer behind the UI commands.
//!
//! `AppService` implements every command of `docs/ipc.md` as a plain async
//! method; `src-tauri` only forwards to it. Events leave through [`EventSink`].

mod error;
mod events;
mod launch;
mod service;
mod types;

#[cfg(test)]
mod tests;

pub use error::{AppError, Res};
pub use events::{
    Event, EventSink, LaunchLogEvent, LaunchProgressEvent, LaunchStateEvent, LogLevelName,
    ProgressPhase,
};
pub use service::{AppService, ServiceOptions};
pub use types::{
    AppState, ConfigView, Latest, LaunchResult, LaunchState, LoaderVersion, ProfileView,
    PublicAccount, VersionEntry, VersionManifest,
};
