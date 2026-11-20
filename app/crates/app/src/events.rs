use crate::types::LaunchState;
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ProgressPhase {
    Client,
    Libraries,
    Assets,
    Java,
    Modpack,
    Done,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LogLevelName {
    Info,
    Warn,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchStateEvent {
    pub profile_id: String,
    pub state: LaunchState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchProgressEvent {
    pub profile_id: String,
    pub phase: ProgressPhase,
    pub done: u64,
    pub total: u64,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchLogEvent {
    pub profile_id: String,
    pub line: String,
    pub level: LogLevelName,
}

/// Rust -> UI events.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    LaunchState(LaunchStateEvent),
    LaunchProgress(LaunchProgressEvent),
    LaunchLog(LaunchLogEvent),
}

impl Event {
    /// The event name on the wire.
    pub fn name(&self) -> &'static str {
        match self {
            Event::LaunchState(_) => "launch:state",
            Event::LaunchProgress(_) => "launch:progress",
            Event::LaunchLog(_) => "launch:log",
        }
    }

    /// The camelCase JSON payload.
    pub fn payload(&self) -> Value {
        let v = match self {
            Event::LaunchState(e) => serde_json::to_value(e),
            Event::LaunchProgress(e) => serde_json::to_value(e),
            Event::LaunchLog(e) => serde_json::to_value(e),
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
