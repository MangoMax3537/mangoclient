use mango_core::{Account, AccountType, Config, Loader, Paths, Profile};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LaunchState {
    Preparing,
    Java,
    Loader,
    Downloading,
    Account,
    Launching,
    Running,
    Stopped,
    Crashed,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileView {
    pub id: String,
    pub name: String,
    pub mc_version: String,
    pub loader: Loader,
    pub loader_version: Option<String>,
    pub color: Option<String>,
    pub cover_url: Option<String>,
    pub ram: Option<u32>,
    pub java_args: String,
    pub mango_config: Option<bool>,
    pub last_played: Option<u64>,
    pub play_time_ms: u64,
    pub created: u64,
    pub mod_count: usize,
}

impl From<&Profile> for ProfileView {
    fn from(p: &Profile) -> Self {
        ProfileView {
            id: p.id.clone(),
            name: p.name.clone(),
            mc_version: p.mc_version.clone(),
            loader: p.loader,
            loader_version: (!p.loader_version.is_empty()).then(|| p.loader_version.clone()),
            color: p.color.clone(),
            cover_url: p.cover.then(|| format!("mangoimg://cover/{}", p.id)),
            ram: p.ram,
            java_args: p.java_args.clone(),
            mango_config: p.mango_config,
            last_played: p.last_played,
            play_time_ms: p.play_time_ms,
            created: p.created,
            mod_count: p.mods.len(),
        }
    }
}

/// Account without any secret.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicAccount {
    pub uuid: String,
    pub name: String,
    #[serde(rename = "type")]
    pub kind: AccountType,
    pub expires_at: Option<i64>,
    pub expired: bool,
    pub has_cape: bool,
}

impl PublicAccount {
    pub fn from_account(a: &Account, now_ms: i64) -> Self {
        let expired = match (a.kind, a.expires_at) {
            (AccountType::Offline, _) | (_, None) | (_, Some(0)) => false,
            (_, Some(e)) => e < now_ms,
        };
        let has_cape = a
            .capes
            .iter()
            .any(|c| c.get("state").and_then(|s| s.as_str()) == Some("ACTIVE"));
        PublicAccount {
            uuid: a.uuid.clone(),
            name: a.name.clone(),
            kind: a.kind,
            expires_at: a.expires_at,
            expired,
            has_cape,
        }
    }
}

/// The config as the UI sees it (no `theme`).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigView {
    pub language: String,
    pub ram: u32,
    pub java_path: String,
    pub java_args: String,
    pub fullscreen: bool,
    pub width: u32,
    pub height: u32,
    pub keep_launcher_open: bool,
    pub hide_on_launch: bool,
    pub show_snapshots: bool,
    pub mango_config: bool,
    pub performance_mods: bool,
    pub concurrent_downloads: u32,
    pub performance_preset: String,
    pub selected_profile: Option<String>,
    pub selected_account: Option<String>,
    pub first_run_done: bool,
    pub telemetry: bool,
    pub sidebar_open: bool,
}

impl From<&Config> for ConfigView {
    fn from(c: &Config) -> Self {
        ConfigView {
            language: c.language.clone(),
            ram: c.ram,
            java_path: c.java_path.clone(),
            java_args: c.java_args.clone(),
            fullscreen: c.fullscreen,
            width: c.width,
            height: c.height,
            keep_launcher_open: c.keep_launcher_open,
            hide_on_launch: c.hide_on_launch,
            show_snapshots: c.show_snapshots,
            mango_config: c.mango_config,
            performance_mods: c.performance_mods,
            concurrent_downloads: c.concurrent_downloads,
            performance_preset: c.performance_preset.clone(),
            selected_profile: c.selected_profile.clone(),
            selected_account: c.selected_account.clone(),
            first_run_done: c.first_run_done,
            telemetry: c.telemetry,
            sidebar_open: c.sidebar_open,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppState {
    pub version: String,
    pub config: ConfigView,
    pub profiles: Vec<ProfileView>,
    pub accounts: Vec<PublicAccount>,
    pub total_ram_mb: u64,
    pub running: Vec<String>,
    pub root_dir: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionEntry {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub release_time: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Latest {
    pub release: String,
    pub snapshot: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionManifest {
    pub latest: Latest,
    pub versions: Vec<VersionEntry>,
    pub installed: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LoaderVersion {
    pub version: String,
    pub stable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchResult {
    pub pid: u32,
    pub version_id: String,
}

/// Names of the version directories that already hold a version json.
pub(crate) fn installed_versions(paths: &Paths) -> Vec<String> {
    let mut out = Vec::new();
    if let Ok(rd) = std::fs::read_dir(paths.versions()) {
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if e.path().join(format!("{name}.json")).is_file() {
                out.push(name);
            }
        }
    }
    out.sort();
    out
}
