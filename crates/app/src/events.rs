use crate::block_icons::BlockIconsStatus;
use crate::models::{ServerStatus, UpdateStatus};
use crate::types::LaunchState;
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use ts_rs::TS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "lowercase")]
pub enum ProgressPhase {
    Client,
    Libraries,
    Assets,
    Java,
    Modpack,
    Done,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "lowercase")]
pub enum LogLevelName {
    Info,
    Warn,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct LaunchStateEvent {
    pub profile_id: String,
    pub state: LaunchState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct LaunchProgressEvent {
    pub profile_id: String,
    pub phase: ProgressPhase,
    pub done: u64,
    pub total: u64,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct LaunchLogEvent {
    pub profile_id: String,
    pub line: String,
    pub level: LogLevelName,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct InstallProgressEvent {
    pub profile_id: String,
    pub project_id: String,
    pub done: u64,
    pub total: u64,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ContentChangedEvent {
    pub profile_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct AccountsChangedEvent {
    pub uuid: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct PanoramaReadyEvent {
    pub profile_id: String,
}

/// A skin, cape or head of this account changed: reload its images.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SkinUpdatedEvent {
    pub uuid: String,
}

/// Progress of a managed Java download (`java_install`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct JavaProgressEvent {
    pub major: u32,
    pub done: u64,
    pub total: u64,
}

/// The launcher window changed state (custom title bar icons).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct WindowStateEvent {
    pub maximized: bool,
    pub fullscreen: bool,
    pub focused: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct FilesDroppedEvent {
    pub paths: Vec<String>,
}

/// Rust -> UI events.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    LaunchState(LaunchStateEvent),
    LaunchProgress(LaunchProgressEvent),
    LaunchLog(LaunchLogEvent),
    InstallProgress(InstallProgressEvent),
    ServersPinged(ServerStatus),
    UpdateStatus(UpdateStatus),
    ContentChanged(ContentChangedEvent),
    AccountsChanged(AccountsChangedEvent),
    BlocksStatus(BlockIconsStatus),
    PanoramaReady(PanoramaReadyEvent),
    FilesDropped(FilesDroppedEvent),
    SkinUpdated(SkinUpdatedEvent),
    JavaProgress(JavaProgressEvent),
    WindowState(WindowStateEvent),
}

impl Event {
    /// The event name on the wire.
    pub fn name(&self) -> &'static str {
        match self {
            Event::LaunchState(_) => "launch:state",
            Event::LaunchProgress(_) => "launch:progress",
            Event::LaunchLog(_) => "launch:log",
            Event::InstallProgress(_) => "install:progress",
            Event::ServersPinged(_) => "servers:pinged",
            Event::UpdateStatus(_) => "update:status",
            Event::ContentChanged(_) => "content:changed",
            Event::AccountsChanged(_) => "accounts:changed",
            Event::BlocksStatus(_) => "blocks:status",
            Event::PanoramaReady(_) => "panorama:ready",
            Event::FilesDropped(_) => "files:dropped",
            Event::SkinUpdated(_) => "skin:updated",
            Event::JavaProgress(_) => "java:progress",
            Event::WindowState(_) => "window:state",
        }
    }

    /// The camelCase JSON payload.
    pub fn payload(&self) -> Value {
        let v = match self {
            Event::LaunchState(e) => serde_json::to_value(e),
            Event::LaunchProgress(e) => serde_json::to_value(e),
            Event::LaunchLog(e) => serde_json::to_value(e),
            Event::InstallProgress(e) => serde_json::to_value(e),
            Event::ServersPinged(e) => serde_json::to_value(e),
            Event::UpdateStatus(e) => serde_json::to_value(e),
            Event::ContentChanged(e) => serde_json::to_value(e),
            Event::AccountsChanged(e) => serde_json::to_value(e),
            Event::BlocksStatus(e) => serde_json::to_value(e),
            Event::PanoramaReady(e) => serde_json::to_value(e),
            Event::FilesDropped(e) => serde_json::to_value(e),
            Event::SkinUpdated(e) => serde_json::to_value(e),
            Event::JavaProgress(e) => serde_json::to_value(e),
            Event::WindowState(e) => serde_json::to_value(e),
        };
        v.unwrap_or(Value::Null)
    }
}

/// Where events go (the Tauri shell implements this with `app.emit`).
pub trait EventSink: Send + Sync + 'static {
    fn emit(&self, event: Event);
}

/// At most one progress event per profile per 100 ms; `force` always passes.
#[derive(Debug, Default)]
pub(crate) struct ProgressThrottle {
    last: HashMap<String, Instant>,
}

impl ProgressThrottle {
    pub(crate) fn ready(&mut self, profile: &str, force: bool, now: Instant) -> bool {
        let due = force
            || self
                .last
                .get(profile)
                .is_none_or(|t| now.saturating_duration_since(*t) >= Duration::from_millis(100));
        if due {
            self.last.insert(profile.to_string(), now);
        }
        due
    }
}
