//! mango-app: Tauri-free service layer behind the UI commands.
//!
//! `AppService` implements every command of `docs/ipc.md` as a plain async
//! method; `src-tauri` only forwards to it. Events leave through [`EventSink`].

mod accounts;
mod background;
mod backgrounds;
mod backups;
pub mod bindings;
mod block_icons;
mod content;
mod crash;
pub mod dispatch;
mod error;
mod events;
mod export;
mod host;
mod icon_images;
pub mod image_protocol;
mod images;
mod import;
mod install;
mod launch;
mod media;
mod models;
mod outfile;
mod path_grants;
mod reports;
mod scrub;
mod servers;
mod service;
mod shell;
mod source;
mod types;
mod window_policy;
mod wire;
mod worlds;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_p10;
#[cfg(test)]
mod tests_p11;
#[cfg(test)]
mod tests_p2;
#[cfg(test)]
mod tests_p3;
#[cfg(test)]
mod tests_p4;
#[cfg(test)]
mod tests_p6;
#[cfg(test)]
mod tests_p7;
#[cfg(test)]
mod tests_p8;
#[cfg(test)]
mod tests_p9;

pub use accounts::LaunchCredentials;
pub use backgrounds::{PanoramaSet, PanoramaSource};
pub use backups::{Backup, BackupKind, BackupReason};
pub use block_icons::{BlockIcon, BlockIconsStatus, BlocksState};
pub use crash::{Confidence, CrashCause, CrashReport};
pub use error::{AppError, Res};
pub use events::{
    AccountsChangedEvent, ContentChangedEvent, Event, EventSink, FilesDroppedEvent,
    InstallProgressEvent, LaunchLogEvent, LaunchProgressEvent, LaunchStateEvent, LogLevelName,
    PanoramaReadyEvent, ProgressPhase,
};
pub use export::ExportFormat;
pub use host::{
    is_login_redirect, login_navigation_allowed, CannedFileDialog, CannedLoginWindow,
    ClipboardImage, ExternalOpener, FileDialog, Host, LoginOutcome, LoginWindow, NoClipboard,
    NoFileDialog, NoLoginWindow, OpenFolder, SystemExternal, SystemOpener,
};
pub use import::{ImportKind, ImportPlan};
pub use models::{
    ContentItem, ContentType, InstallPlan, InstallPlanItem, InstallTarget, LogEntry, LogKind,
    LogText, LogUpload, ModpackPlan, PlanConflict, PlanRole, ProjectDetail, ProjectHit,
    ProjectVersion, Screenshot, ScreenshotAction, SearchParams, SearchResult, ServerEntry,
    ServerPlayers, ServerStatus, StatsDay, StatsProfile, StatsSummary, StorageKey, StorageUsage,
    TargetReason, TargetState, ToggleResult, UninstallResult, UpdateInfo, UpdateStatus,
};
pub use outfile::FileExport;
pub use path_grants::DEFAULT_GRANT_TTL;
pub use service::{resolve_bundle_dir, updates_dir, AppService, ServiceOptions};
pub use shell::Activity;
pub use types::{
    AppState, ConfigView, InstanceIcon, Latest, LaunchResult, LaunchState, LoaderVersion,
    ProfileBackground, ProfileView, PublicAccount, VersionEntry, VersionManifest,
};
pub use window_policy::{PolicyInput, WindowAction, WindowPolicy};
pub use wire::{
    BlockFaces, ConfigPatch, Language, PerformancePreset, ProfilePatch, ProjectKind, SearchSort,
};
pub use worlds::{GameMode, World};
