//! Wire types of `docs/ipc.md` Part 2 (camelCase JSON).

use crate::wire::{ProjectKind, SearchSort};
use mango_core::ModType;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// `mod | shader | resourcepack | datapack`. Shares the stored type's names.
pub type ContentType = ModType;

/// Where a project comes from. A project id is only unique together with it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    #[default]
    Modrinth,
    Curseforge,
}

impl Provider {
    pub fn as_str(self) -> &'static str {
        match self {
            Provider::Modrinth => "modrinth",
            Provider::Curseforge => "curseforge",
        }
    }

    /// The provider a stored record belongs to. Records written before
    /// providers existed (no `provider`) are Modrinth's. Local jars have none.
    pub fn of_record(rec: &mango_core::ModRecord) -> Option<Provider> {
        if rec.local == Some(true) {
            return None;
        }
        Some(match rec.provider.as_deref() {
            Some("curseforge") => Provider::Curseforge,
            _ => Provider::Modrinth,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ContentItem {
    /// `null` = local jar.
    pub provider: Option<Provider>,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ProjectHit {
    pub provider: Provider,
    pub project_id: String,
    pub slug: String,
    pub title: String,
    pub author: String,
    pub description: String,
    /// A content type or `modpack`.
    #[serde(rename = "type")]
    #[ts(as = "ProjectKind")]
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    pub hits: Vec<ProjectHit>,
    pub total: u64,
    pub offset: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ProjectVersion {
    pub provider: Provider,
    pub version_id: String,
    pub version_number: String,
    pub name: String,
    /// `release | beta | alpha`.
    #[serde(rename = "type")]
    #[ts(type = "'release' | 'beta' | 'alpha'")]
    pub kind: String,
    pub game_versions: Vec<String>,
    pub loaders: Vec<String>,
    pub date_published: String,
    pub downloads: u64,
    pub compatible: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    /// `null` = local jar (never reported, kept for the shared shape).
    pub provider: Option<Provider>,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ToggleResult {
    pub items: Vec<ContentItem>,
    /// Project ids whose state changed (the item itself and its tree).
    pub affected: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct UninstallResult {
    pub blocked: bool,
    /// Project ids of the installed items that require the project.
    pub dependents: Vec<String>,
    pub items: Vec<ContentItem>,
}

/// UI -> Rust search arguments.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SearchParams {
    /// The source; Modrinth when omitted.
    #[serde(default)]
    #[ts(as = "Option<Provider>", optional)]
    pub provider: Provider,
    #[serde(default)]
    #[ts(as = "Option<String>", optional)]
    pub query: String,
    /// A content type or `modpack`.
    #[ts(as = "ProjectKind")]
    pub project_type: String,
    #[serde(default)]
    #[ts(as = "Option<mango_core::Loader>", optional = nullable)]
    pub loader: Option<String>,
    #[serde(default)]
    #[ts(optional = nullable)]
    pub mc_version: Option<String>,
    #[serde(default)]
    #[ts(optional = nullable)]
    pub categories: Option<Vec<String>>,
    #[serde(default = "default_sort")]
    #[ts(as = "Option<SearchSort>", optional)]
    pub sort: String,
    #[serde(default)]
    #[ts(as = "Option<u32>", optional)]
    pub offset: u32,
    #[serde(default = "default_limit")]
    #[ts(as = "Option<u32>", optional)]
    pub limit: u32,
}

fn default_sort() -> String {
    "relevance".into()
}

fn default_limit() -> u32 {
    30
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ServerEntry {
    pub id: String,
    pub name: String,
    pub address: String,
    pub partner: bool,
    pub starred: bool,
    pub icon_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct ServerPlayers {
    pub online: i64,
    pub max: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ServerStatus {
    pub id: String,
    pub online: bool,
    pub ping: Option<u64>,
    pub motd: String,
    pub players: Option<ServerPlayers>,
    pub version: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Screenshot {
    pub name: String,
    pub size: u64,
    /// Modification time, epoch ms.
    pub taken: u64,
    pub url: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "lowercase")]
pub enum LogKind {
    Game,
    Crash,
    Launcher,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct LogEntry {
    pub id: String,
    pub name: String,
    pub size: u64,
    pub modified: u64,
    pub kind: LogKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct LogText {
    pub text: String,
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct LogUpload {
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct StatsDay {
    pub date: String,
    pub ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct StatsProfile {
    pub profile_id: String,
    pub name: String,
    pub ms: u64,
    pub sessions: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct StatsSummary {
    pub days: Vec<StatsDay>,
    pub per_profile: Vec<StatsProfile>,
    pub total_ms: u64,
    pub sessions: u64,
    pub longest_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct StorageUsage {
    pub key: StorageKey,
    pub bytes: u64,
    pub clearable: bool,
}

/// `update:status` payload and the return of `update_status|update_check`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct UpdateStatus {
    #[ts(
        type = "'idle' | 'checking' | 'downloading' | 'ready' | 'current' | 'error' | 'disabled'"
    )]
    pub state: &'static str,
    #[ts(optional)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub percent: Option<u8>,
    #[ts(optional)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[ts(optional)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    #[ts(optional)]
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(type = "'no-release' | 'offline' | 'other'")]
    pub error_code: Option<&'static str>,
    #[ts(optional)]
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(type = "'dev' | 'unsupported'")]
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

// ---------- Part 4: providers, install planning, modpacks ----------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum StatusReason {
    NoApiKey,
    Offline,
    Disabled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ProviderStatus {
    pub provider: Provider,
    pub available: bool,
    #[ts(optional)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<StatusReason>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum TargetState {
    Available,
    Installed,
    Incompatible,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum TargetReason {
    Loader,
    McVersion,
    ContentType,
    NoCompatibleVersion,
    DistributionBlocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct InstallTarget {
    pub profile_id: String,
    pub state: TargetState,
    pub installed_version: Option<String>,
    pub reason: Option<TargetReason>,
    pub detail: Option<String>,
    pub required_deps: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum PlanRole {
    Requested,
    Required,
    Optional,
    Incompatible,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct InstallPlanItem {
    pub provider: Provider,
    pub project_id: String,
    pub version_id: String,
    pub title: String,
    #[serde(rename = "type")]
    pub kind: ContentType,
    pub icon_url: Option<String>,
    pub role: PlanRole,
    pub already_installed: bool,
    pub installed_version: Option<String>,
    pub distribution_blocked: bool,
    pub size_bytes: Option<u64>,
    /// Only set when `distribution_blocked`: the page the user must use.
    pub download_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct PlanConflict {
    pub project_id: String,
    pub title: String,
    /// Title of the item that declares the incompatibility.
    pub with: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct InstallPlan {
    pub profile_id: String,
    pub items: Vec<InstallPlanItem>,
    /// Bytes the default selection would download (requested + required items
    /// that are not installed yet, minus blocked ones).
    pub total_bytes: u64,
    pub conflicts: Vec<PlanConflict>,
}

/// Where a blocked pack file stands (Part 5 of `docs/ipc.md`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum BlockedState {
    /// The user has to fetch it from the CurseForge page.
    Pending,
    /// Found on Modrinth by hash: will be, or was, installed from there.
    MatchedModrinth,
    /// A matching file turned up in a watched folder; being copied.
    Found,
    Installed,
    Skipped,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct BlockedModrinth {
    pub project_id: String,
    pub version_id: String,
    pub title: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct BlockedFile {
    /// Display name (the project name when known, else the file name).
    pub name: String,
    /// The bare file name the pack expects; the key for `blocked_skip`.
    pub file_name: String,
    /// The CurseForge page to fetch it from.
    pub url: String,
    pub sha1: Option<String>,
    pub size: Option<u64>,
    #[serde(rename = "type")]
    pub kind: ContentType,
    pub state: BlockedState,
    pub modrinth: Option<BlockedModrinth>,
    pub found_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ModpackPlan {
    pub provider: Provider,
    pub project_id: String,
    pub version_id: String,
    pub name: String,
    pub mc_version: String,
    pub loader: mango_core::Loader,
    pub loader_version: Option<String>,
    pub file_count: u64,
    pub size_bytes: Option<u64>,
    pub icon_url: Option<String>,
    pub blocked_files: Vec<BlockedFile>,
}
