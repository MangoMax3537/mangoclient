use crate::error::ErrorCode;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DisabledReason {
    Dev,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "kebab-case")]
pub enum UpdateState {
    Idle,
    Checking,
    Downloading { version: String, percent: u8 },
    Ready { version: String },
    Current,
    Error { code: ErrorCode, message: String },
    Disabled { reason: DisabledReason },
}

/// What UI subscribers receive over the watch channel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateStatus {
    #[serde(flatten)]
    pub state: UpdateState,
    /// Running version.
    pub current: String,
    /// The launch check will relaunch into the update once it is ready.
    pub applying: bool,
}
