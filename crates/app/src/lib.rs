//! mango-app: Tauri-free service layer behind the UI commands.
//!
//! `AppService` implements every command of `docs/ipc.md` as a plain async
//! method; `src-tauri` only forwards to it. Events leave through [`EventSink`].

mod accounts;
mod background;
mod backgrounds;
pub mod bindings;
mod block_icons;
mod blocked;
mod content;
pub mod dispatch;
mod error;
mod events;
mod host;
mod icon_images;
pub mod image_protocol;
mod images;
mod install;
mod launch;
mod media;
mod models;
mod providers;
mod servers;
mod service;
mod types;
mod wire;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_p2;
#[cfg(test)]
mod tests_p3;
#[cfg(test)]
mod tests_p4;
#[cfg(test)]
mod tests_p5;
#[cfg(test)]
mod tests_p6;
#[cfg(test)]
mod tests_p7;
#[cfg(test)]
mod tests_p8;

pub use accounts::LaunchCredentials;
pub use backgrounds::{PanoramaSet, PanoramaSource};
pub use block_icons::{BlockIcon, BlockIconsStatus, BlocksState};
pub use error::{AppError, Res};
pub use events::{
    AccountsChangedEvent, BlockedChangedEvent, ContentChangedEvent, Event, EventSink,
    InstallProgressEvent, LaunchLogEvent, LaunchProgressEvent, LaunchStateEvent, LogLevelName,
    PanoramaReadyEvent, ProgressPhase,
};
pub use host::{
    is_login_redirect, login_navigation_allowed, CannedFileDialog, CannedLoginWindow,
    ClipboardImage, ExternalOpener, FileDialog, Host, LoginOutcome, LoginWindow, NoClipboard,
    NoFileDialog, NoLoginWindow, OpenFolder, SystemExternal, SystemOpener,
};
pub use models::{
    BlockedFile, BlockedModrinth, BlockedState, ContentItem, ContentType, InstallPlan,
    InstallPlanItem, InstallTarget, LogEntry, LogKind, LogText, LogUpload, ModpackPlan,
    PlanConflict, PlanRole, ProjectDetail, ProjectHit, ProjectVersion, Provider, ProviderStatus,
    Screenshot, ScreenshotAction, SearchParams, SearchResult, ServerEntry, ServerPlayers,
    ServerStatus, StatsDay, StatsProfile, StatsSummary, StatusReason, StorageKey, StorageUsage,
    TargetReason, TargetState, ToggleResult, UninstallResult, UpdateInfo, UpdateStatus,
};
pub use service::{resolve_bundle_dir, updates_dir, AppService, ServiceOptions};
pub use types::{
    AppState, ConfigView, InstanceIcon, Latest, LaunchResult, LaunchState, LoaderVersion,
    ProfileBackground, ProfileView, PublicAccount, VersionEntry, VersionManifest,
};
pub use wire::{
    BlockFaces, ConfigPatch, Language, PerformancePreset, ProfilePatch, ProjectKind, SearchSort,
};
