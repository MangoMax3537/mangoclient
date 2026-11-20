//! Wire types of `docs/ipc.md` Part 2 (camelCase JSON).

use mango_core::ModType;
use serde::{Deserialize, Serialize};

/// `mod | shader | resourcepack | datapack`. Shares the stored type's names.
pub type ContentType = ModType;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentItem {
    pub project_id: String,
    pub slug: Option<String>,
    pub title: String,
    #[serde(rename = "type")]
    pub kind: ContentType,
    pub version_id: Option<String>,
    pub version_number: Option<String>,
    pub filename: String,
    pub enabled: bool,
    pub local: bool,
    pub icon_url: Option<String>,
    /// Project ids of installed items that require this one.
    pub required_by: Vec<String>,
    pub dependency: bool,
    pub update_available: Option<String>,
    pub installed_at: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectHit {
    pub project_id: String,
    pub slug: String,
    pub title: String,
    pub author: String,
    pub description: String,
    /// A content type or `modpack`.
    #[serde(rename = "type")]
    pub kind: String,
    pub icon_url: Option<String>,
    pub downloads: u64,
    pub follows: u64,
    pub categories: Vec<String>,
    pub game_versions: Vec<String>,
    pub loaders: Vec<String>,
    pub date_modified: String,
    /// Ids of the profiles that have this project installed.
    pub installed_in: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectDetail {
    #[serde(flatten)]
    pub hit: ProjectHit,
    pub body: String,
    pub gallery: Vec<String>,
    pub license: Option<String>,
    pub source_url: Option<String>,
    pub issues_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    pub hits: Vec<ProjectHit>,
    pub total: u64,
    pub offset: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectVersion {
    pub version_id: String,
    pub version_number: String,
    pub name: String,
    /// `release | beta | alpha`.
    #[serde(rename = "type")]
    pub kind: String,
    pub game_versions: Vec<String>,
    pub loaders: Vec<String>,
    pub date_published: String,
    pub downloads: u64,
    pub compatible: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub project_id: String,
    pub title: String,
    #[serde(rename = "type")]
    pub kind: ContentType,
    pub from: Option<String>,
    pub to: String,
    pub version_id: String,
    pub compatibility: bool,
    pub conflict: bool,
    /// Project ids.
    pub required_by: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToggleResult {
    pub items: Vec<ContentItem>,
    /// Project ids whose state changed (the item itself and its tree).
    pub affected: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UninstallResult {
    pub blocked: bool,
    /// Project ids of the installed items that require the project.
    pub dependents: Vec<String>,
    pub items: Vec<ContentItem>,
}

/// UI -> Rust search arguments.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchParams {
    #[serde(default)]
    pub query: String,
    /// A content type or `modpack`.
    pub project_type: String,
    #[serde(default)]
    pub loader: Option<String>,
    #[serde(default)]
    pub mc_version: Option<String>,
    #[serde(default)]
    pub categories: Option<Vec<String>>,
    #[serde(default = "default_sort")]
    pub sort: String,
    #[serde(default)]
    pub offset: u32,
    #[serde(default = "default_limit")]
    pub limit: u32,
}

fn default_sort() -> String {
    "relevance".into()
}

fn default_limit() -> u32 {
    30
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerEntry {
    pub id: String,
    pub name: String,
    pub address: String,
    pub partner: bool,
    pub starred: bool,
    pub icon_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ServerPlayers {
    pub online: i64,
    pub max: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerStatus {
    pub id: String,
    pub online: bool,
    pub ping: Option<u64>,
    pub motd: String,
    pub players: Option<ServerPlayers>,
    pub version: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Screenshot {
    pub name: String,
    pub size: u64,
    /// Modification time, epoch ms.
    pub taken: u64,
    pub url: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ScreenshotAction {
    Delete,
    Reveal,
    Copy,
}

impl<'de> Deserialize<'de> for ScreenshotAction {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        match s.as_str() {
            "delete" => Ok(Self::Delete),
            "reveal" => Ok(Self::Reveal),
            "copy" => Ok(Self::Copy),
            other => Err(serde::de::Error::custom(format!(
                "unknown screenshot action {other}"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LogKind {
    Game,
    Crash,
    Launcher,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogEntry {
    pub id: String,
    pub name: String,
    pub size: u64,
    pub modified: u64,
    pub kind: LogKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogText {
    pub text: String,
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LogUpload {
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StatsDay {
    pub date: String,
    pub ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatsProfile {
    pub profile_id: String,
    pub name: String,
    pub ms: u64,
    pub sessions: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatsSummary {
    pub days: Vec<StatsDay>,
    pub per_profile: Vec<StatsProfile>,
    pub total_ms: u64,
    pub sessions: u64,
    pub longest_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum StorageKey {
    Instances,
    Versions,
    Libraries,
    Assets,
    Runtimes,
    Natives,
    Cache,
    Logs,
}

impl<'de> Deserialize<'de> for StorageKey {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        StorageKey::ALL
            .into_iter()
            .find(|k| k.as_str() == s)
            .ok_or_else(|| serde::de::Error::custom(format!("unknown storage key {s}")))
    }
}

impl StorageKey {
    pub const ALL: [StorageKey; 8] = [
        StorageKey::Instances,
        StorageKey::Versions,
        StorageKey::Libraries,
        StorageKey::Assets,
        StorageKey::Runtimes,
        StorageKey::Natives,
        StorageKey::Cache,
        StorageKey::Logs,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            StorageKey::Instances => "instances",
            StorageKey::Versions => "versions",
            StorageKey::Libraries => "libraries",
            StorageKey::Assets => "assets",
            StorageKey::Runtimes => "runtimes",
            StorageKey::Natives => "natives",
            StorageKey::Cache => "cache",
            StorageKey::Logs => "logs",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageUsage {
    pub key: StorageKey,
    pub bytes: u64,
    pub clearable: bool,
}

/// `update:status` payload and the return of `update_status|update_check`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateStatus {
    pub state: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub percent: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disabled_reason: Option<&'static str>,
}

impl UpdateStatus {
    fn bare(state: &'static str) -> Self {
        UpdateStatus {
            state,
            percent: None,
            version: None,
            notes: None,
            error_code: None,
            disabled_reason: None,
        }
    }
}

impl From<&mango_updater::UpdateStatus> for UpdateStatus {
    fn from(s: &mango_updater::UpdateStatus) -> Self {
        use mango_updater::{DisabledReason, ErrorCode, UpdateState};
        match &s.state {
            UpdateState::Idle => UpdateStatus::bare("idle"),
            UpdateState::Checking => UpdateStatus::bare("checking"),
            UpdateState::Current => UpdateStatus::bare("current"),
            UpdateState::Downloading { version, percent } => UpdateStatus {
                percent: Some(*percent),
                version: Some(version.clone()),
                ..UpdateStatus::bare("downloading")
            },
            UpdateState::Ready { version } => UpdateStatus {
                version: Some(version.clone()),
                ..UpdateStatus::bare("ready")
            },
            UpdateState::Error { code, .. } => UpdateStatus {
                error_code: Some(match code {
                    ErrorCode::NoRelease => "no-release",
                    ErrorCode::Offline => "offline",
                    ErrorCode::Other => "other",
                }),
                ..UpdateStatus::bare("error")
            },
            UpdateState::Disabled { reason } => UpdateStatus {
                disabled_reason: Some(match reason {
                    DisabledReason::Dev => "dev",
                    DisabledReason::Unsupported => "unsupported",
                }),
                ..UpdateStatus::bare("disabled")
            },
        }
    }
}
