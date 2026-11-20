//! mango-app: Tauri-free service layer behind the UI commands.
//!
//! `AppService` implements every command of `docs/ipc.md` as a plain async
//! method; `src-tauri` only forwards to it. Events leave through [`EventSink`].

mod background;
mod content;
mod error;
mod events;
mod host;
pub mod image_protocol;
mod images;
mod launch;
mod media;
mod models;
mod servers;
mod service;
mod types;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_p2;

pub use error::{AppError, Res};
pub use events::{
    ContentChangedEvent, Event, EventSink, InstallProgressEvent, LaunchLogEvent,
    LaunchProgressEvent, LaunchStateEvent, LogLevelName, ProgressPhase,
};
pub use host::{ClipboardImage, Host, NoClipboard, OpenFolder, SystemOpener};
pub use models::{
    ContentItem, ContentType, LogEntry, LogKind, LogText, LogUpload, ProjectDetail, ProjectHit,
    ProjectVersion, Screenshot, ScreenshotAction, SearchParams, SearchResult, ServerEntry,
    ServerPlayers, ServerStatus, StatsDay, StatsProfile, StatsSummary, StorageKey, StorageUsage,
    ToggleResult, UninstallResult, UpdateInfo, UpdateStatus,
};
pub use service::{AppService, ServiceOptions};
pub use types::{
    AppState, ConfigView, Latest, LaunchResult, LaunchState, LoaderVersion, ProfileView,
    PublicAccount, VersionEntry, VersionManifest,
};
