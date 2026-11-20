use mango_core::{Account, AccountType, Config, Loader, Paths, Profile};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
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

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
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
    /// Blocked pack files still waiting for the user (pending or found).
    pub pending_blocked: usize,
    /// `null` = automatic.
    pub icon: Option<InstanceIcon>,
    /// The block the automatic rule picks (gallery ready), else `null`.
    pub auto_block: Option<String>,
    /// `custom` | `modpack` while the icon is an image, else `null`.
    #[ts(type = "'custom' | 'modpack' | null")]
    pub icon_source: Option<String>,
    pub background: ProfileBackground,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum InstanceIcon {
    Image,
    Block { block: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum ProfileBackground {
    /// The version's panorama. `dim` (0..1) and `blur` (px, 0..24) override
    /// the defaults when present.
    Auto {
        #[ts(optional)]
        #[serde(skip_serializing_if = "Option::is_none")]
        dim: Option<f64>,
        #[ts(optional)]
        #[serde(skip_serializing_if = "Option::is_none")]
        blur: Option<f64>,
    },
    /// A wallpaper bundled with the UI (`id` = file stem).
    Wallpaper {
        id: String,
        #[ts(optional)]
        #[serde(skip_serializing_if = "Option::is_none")]
        dim: Option<f64>,
        #[ts(optional)]
        #[serde(skip_serializing_if = "Option::is_none")]
        blur: Option<f64>,
    },
    /// Explicit panorama choice (same picture as `auto`).
    Panorama {
        #[ts(optional)]
        #[serde(skip_serializing_if = "Option::is_none")]
        dim: Option<f64>,
        #[ts(optional)]
        #[serde(skip_serializing_if = "Option::is_none")]
        blur: Option<f64>,
    },
    Image {
        url: String,
        #[ts(optional)]
        #[serde(skip_serializing_if = "Option::is_none")]
        dim: Option<f64>,
        #[ts(optional)]
        #[serde(skip_serializing_if = "Option::is_none")]
        blur: Option<f64>,
    },
    None,
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
            cover_url: p.cover.then(|| crate::image_protocol::cover_url(&p.id)),
            ram: p.ram,
            java_args: p.java_args.clone(),
            mango_config: p.mango_config,
            last_played: p.last_played,
            play_time_ms: p.play_time_ms,
            created: p.created,
            mod_count: p.mods.len(),
            pending_blocked: 0,
            icon: if p.cover {
                Some(InstanceIcon::Image)
            } else {
                p.icon_block
                    .clone()
                    .map(|block| InstanceIcon::Block { block })
            },
            auto_block: None,
            icon_source: p
                .cover
                .then(|| p.icon_source.clone().unwrap_or_else(|| "custom".into())),
            background: match p.background.as_deref() {
                Some("none") => ProfileBackground::None,
                Some("panorama") => ProfileBackground::Panorama {
                    dim: p.background_dim,
                    blur: p.background_blur,
                },
                Some(w)
                    if w.strip_prefix("wallpaper:")
                        .is_some_and(mango_core::is_wallpaper_id) =>
                {
                    ProfileBackground::Wallpaper {
                        id: w["wallpaper:".len()..].to_string(),
                        dim: p.background_dim,
                        blur: p.background_blur,
                    }
                }
                Some("image") => ProfileBackground::Image {
                    url: crate::image_protocol::cover_url(&format!("{}-bg", p.id)),
                    dim: p.background_dim,
                    blur: p.background_blur,
                },
                _ => ProfileBackground::Auto {
                    dim: p.background_dim,
                    blur: p.background_blur,
                },
            },
        }
    }
}

/// Account without any secret.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct PublicAccount {
    pub uuid: String,
    pub name: String,
    #[serde(rename = "type")]
    pub kind: AccountType,
    pub expires_at: Option<i64>,
    pub expired: bool,
    pub has_cape: bool,
    /// `mangoimg://head/<uuid>`.
    pub head_url: String,
    /// False = memory-only session (keystore unavailable): the login will not
    /// survive a restart.
    pub credentials_persisted: bool,
    /// Keystore locked or envelope undecryptable.
    pub credentials_locked: bool,
    /// No usable refresh token: sign in again.
    pub needs_relogin: bool,
}

/// Credential state of one account (never the secrets themselves).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CredState {
    pub persisted: bool,
    pub locked: bool,
    pub needs_relogin: bool,
}

impl PublicAccount {
    pub fn from_account(a: &Account, now_ms: i64, creds: CredState) -> Self {
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
            head_url: crate::image_protocol::head_url(&a.uuid),
            credentials_persisted: creds.persisted,
            credentials_locked: creds.locked,
            needs_relogin: creds.needs_relogin,
        }
    }
}

/// The config as the UI sees it (no `theme`).
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ConfigView {
    #[ts(type = "'de' | 'en'")]
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
    #[ts(type = "'potato' | 'balanced' | 'quality'")]
    pub performance_preset: String,
    pub selected_profile: Option<String>,
    pub selected_account: Option<String>,
    pub first_run_done: bool,
    pub telemetry: bool,
    pub sidebar_open: bool,
    pub blocked_mods_modrinth_fallback: bool,
    pub blocked_mods_watch_dirs: Vec<String>,
    pub blocked_mods_check_subfolders: bool,
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
            blocked_mods_modrinth_fallback: c.blocked_mods_modrinth_fallback,
            blocked_mods_watch_dirs: c.blocked_mods_watch_dirs.clone(),
            blocked_mods_check_subfolders: c.blocked_mods_check_subfolders,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct VersionEntry {
    pub id: String,
    #[serde(rename = "type")]
    #[ts(type = "'release' | 'snapshot' | 'old_beta' | 'old_alpha'")]
    pub kind: String,
    pub release_time: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct Latest {
    pub release: String,
    pub snapshot: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct VersionManifest {
    pub latest: Latest,
    pub versions: Vec<VersionEntry>,
    pub installed: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct LoaderVersion {
    pub version: String,
    pub stable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
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
