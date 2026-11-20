//! Serde models for config.json and the profiles/accounts in accounts.json.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::error::Result;
use crate::validation::{config_field, config_patch, profile_field};

/// Suggest a heap size: half of RAM in 512 MB steps, clamped to 2048..=8192.
pub fn default_ram_mb(total_mb: u64) -> u32 {
    let half = total_mb / 2 / 512 * 512;
    half.clamp(2048, 8192) as u32
}

/// Best-effort total system RAM (Linux only; other platforms report 8192).
pub fn total_ram_mb() -> u64 {
    if let Ok(s) = std::fs::read_to_string("/proc/meminfo") {
        for line in s.lines() {
            if let Some(rest) = line.strip_prefix("MemTotal:") {
                let kb: u64 = rest
                    .trim()
                    .trim_end_matches("kB")
                    .trim()
                    .parse()
                    .unwrap_or(0);
                if kb > 0 {
                    return kb / 1024;
                }
            }
        }
    }
    8192
}

/// A string that never shows up in `Debug` output (API keys). Serializes as a
/// plain string so it can live in `config.json`.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Secret(String);

impl Secret {
    pub fn new(value: impl Into<String>) -> Self {
        Secret(value.into())
    }

    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Secret(<redacted>)")
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    pub language: String,
    pub theme: String,
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
    pub sidebar_open: bool,
    pub concurrent_downloads: u32,
    pub performance_preset: String,
    pub selected_profile: Option<String>,
    pub selected_account: Option<String>,
    pub first_run_done: bool,
    pub telemetry: bool,
    /// Optional CurseForge API key supplied by the user. Never sent to the UI.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub curseforge_api_key: Option<Secret>,
    /// Look blocked CurseForge pack files up on Modrinth by hash.
    #[serde(default = "default_true")]
    pub blocked_mods_modrinth_fallback: bool,
    /// Folders watched for files the user downloads by hand.
    #[serde(default)]
    pub blocked_mods_watch_dirs: Vec<String>,
    /// Look into sub-folders of the watched folders too.
    #[serde(default)]
    pub blocked_mods_check_subfolders: bool,
}

fn default_true() -> bool {
    true
}

impl Config {
    pub fn with_total_ram(total_mb: u64) -> Self {
        Config {
            language: "de".into(),
            theme: "mango".into(),
            ram: default_ram_mb(total_mb),
            java_path: String::new(),
            java_args: String::new(),
            fullscreen: false,
            width: 1280,
            height: 720,
            keep_launcher_open: true,
            hide_on_launch: false,
            show_snapshots: false,
            mango_config: true,
            performance_mods: true,
            sidebar_open: true,
            concurrent_downloads: 12,
            performance_preset: "balanced".into(),
            selected_profile: None,
            selected_account: None,
            first_run_done: false,
            telemetry: true,
            curseforge_api_key: None,
            blocked_mods_modrinth_fallback: true,
            blocked_mods_watch_dirs: crate::paths::default_downloads_dir()
                .map(|d| d.display().to_string())
                .into_iter()
                .collect(),
            blocked_mods_check_subfolders: false,
        }
    }

    /// Per-key tolerant load: an invalid or unknown key falls back to default.
    pub fn from_value(raw: &Value, total_mb: u64) -> Self {
        let mut config = Config::with_total_ram(total_mb);
        let Some(obj) = raw.as_object() else {
            return config;
        };
        let mut clean = Map::new();
        for (key, value) in obj {
            if let Some(Ok(v)) = config_field(key, value) {
                clean.insert(key.clone(), v);
            }
        }
        // Validated keys always fit the typed struct.
        let _ = config.merge(clean);
        config
    }

    fn merge(&mut self, patch: Map<String, Value>) -> Result<()> {
        let Value::Object(mut current) = serde_json::to_value(&*self)? else {
            return Ok(());
        };
        current.extend(patch);
        *self = serde_json::from_value(Value::Object(current))?;
        Ok(())
    }

    /// Validate and apply a patch from the UI. Nothing changes on error.
    pub fn apply_patch(&mut self, patch: &Value) -> Result<()> {
        let clean = config_patch(patch)?;
        self.merge(clean)
    }
}

impl Default for Config {
    fn default() -> Self {
        Config::with_total_ram(total_ram_mb())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Loader {
    Vanilla,
    Fabric,
    Quilt,
    Forge,
    Neoforge,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModType {
    Mod,
    Shader,
    Resourcepack,
    Datapack,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModDependency {
    pub project_id: String,
    pub version_id: Option<String>,
}

/// An installed content record. `file` is an absolute path, kept as stored.
/// Unknown fields survive a load/save round trip.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModRecord {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slug: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version_number: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filename: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(default, rename = "type", skip_serializing_if = "Option::is_none")]
    pub kind: Option<ModType>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dependency: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub local: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub installed_at: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub game_versions: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loaders: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub required_dependencies: Option<Vec<ModDependency>>,
    /// `modrinth` or `curseforge`. Absent means `modrinth` (records written
    /// before providers existed).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub pid: u32,
    pub version_id: String,
    pub started_at: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub mc_version: String,
    pub loader: Loader,
    pub loader_version: String,
    pub color: Option<String>,
    /// Derived from the existence of `covers/<id>.png`.
    pub cover: bool,
    /// Chosen gallery block (`grass_block`); `None` = automatic.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon_block: Option<String>,
    /// `none` or `image`; `None` = automatic (the version's panorama).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background: Option<String>,
    pub ram: Option<u32>,
    pub java_args: String,
    pub mods: Vec<ModRecord>,
    pub mango_config: Option<bool>,
    pub session: Option<Session>,
    pub last_played: Option<u64>,
    pub play_time_ms: u64,
    pub created: u64,
}

impl Profile {
    pub fn new(id: String, created: u64) -> Self {
        Profile {
            id,
            name: "New Profile".into(),
            mc_version: "1.21.11".into(),
            loader: Loader::Vanilla,
            loader_version: String::new(),
            color: None,
            cover: false,
            icon_block: None,
            background: None,
            ram: None,
            java_args: String::new(),
            mods: Vec::new(),
            mango_config: None,
            session: None,
            last_played: None,
            play_time_ms: 0,
            created,
        }
    }

    /// Merge an already validated patch (see `profile_patch`).
    pub fn apply_clean_patch(&mut self, patch: Map<String, Value>) -> Result<()> {
        let Value::Object(mut current) = serde_json::to_value(&*self)? else {
            return Ok(());
        };
        current.extend(patch);
        *self = serde_json::from_value(Value::Object(current))?;
        Ok(())
    }

    /// Per-key tolerant parse of a stored profile (invalid keys keep their
    /// default). `None` when the id is not a UUID. Mod records are validated
    /// individually; ones that fail validation are dropped, unknown fields are
    /// preserved. Path safety and `cover` are applied by the store.
    pub fn from_stored(raw: &Value, now_ms: u64) -> Option<Profile> {
        let obj = raw.as_object()?;
        let id = obj.get("id")?.as_str()?;
        if !crate::validation::is_uuid(id) {
            return None;
        }
        let mut p = Profile::new(id.to_string(), now_ms);
        let mut clean = Map::new();
        for (key, value) in obj {
            if key == "id" || key == "mods" {
                continue;
            }
            if let Some(Ok(v)) = profile_field(key, value) {
                clean.insert(key.clone(), v);
            }
        }
        let _ = p.apply_clean_patch(clean);
        if let Some(Value::Array(mods)) = obj.get("mods") {
            p.mods = mods.iter().filter_map(stored_mod).collect();
        }
        if let Some(c) = obj
            .get("created")
            .and_then(|v| v.as_u64())
            .filter(|c| *c <= 9_007_199_254_740_991)
        {
            p.created = c;
        }
        Some(p)
    }
}

fn stored_mod(raw: &Value) -> Option<ModRecord> {
    let mut clean = crate::validation::validate_mod(raw).ok()?;
    clean.extend(crate::validation::unknown_mod_keys(raw));
    serde_json::from_value(Value::Object(clean)).ok()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AccountType {
    Microsoft,
    Offline,
}

/// Plain account record. Credential encryption lives elsewhere; the encrypted
/// blob is passed through untouched.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    #[serde(rename = "type")]
    pub kind: AccountType,
    pub uuid: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub access_token: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub msa_refresh_token: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<i64>,
    #[serde(default)]
    pub skins: Vec<Value>,
    #[serde(default)]
    pub capes: Vec<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub encrypted_credentials: Option<String>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn default_ram_rules() {
        assert_eq!(default_ram_mb(4096), 2048);
        assert_eq!(default_ram_mb(16384), 8192);
        assert_eq!(default_ram_mb(32768), 8192);
        assert_eq!(default_ram_mb(10000), 4608);
    }

    #[test]
    fn config_invalid_keys_fall_back_to_default() {
        let c = Config::from_value(
            &json!({"language": "fr", "ram": 4096, "width": 1, "fullscreen": true, "bogus": 1}),
            16384,
        );
        assert_eq!(c.language, "de");
        assert_eq!(c.ram, 4096);
        assert_eq!(c.width, 1280);
        assert!(c.fullscreen);
        assert_eq!(c.concurrent_downloads, 12);
        assert_eq!(Config::from_value(&json!("junk"), 16384).ram, 8192);
    }

    #[test]
    fn config_patch_is_atomic() {
        let mut c = Config::with_total_ram(16384);
        assert!(c.apply_patch(&json!({"language": "en", "ram": 1})).is_err());
        assert_eq!(c.language, "de");
        c.apply_patch(&json!({"language": "en"})).unwrap();
        assert_eq!(c.language, "en");
    }

    #[test]
    fn mod_record_keeps_unknown_fields_and_absolute_path() {
        let raw = json!({"title": "t", "file": "/abs/old/x.jar", "future": {"a": 1}});
        let m: ModRecord = serde_json::from_value(raw.clone()).unwrap();
        assert_eq!(m.file.as_deref(), Some("/abs/old/x.jar"));
        assert_eq!(serde_json::to_value(&m).unwrap(), raw);
    }

    #[test]
    fn account_roundtrip_with_encrypted_passthrough() {
        let raw = json!({"type": "microsoft", "uuid": "u", "name": "n", "skins": [],
            "capes": [], "encryptedCredentials": "blob==", "extraField": 3});
        let a: Account = serde_json::from_value(raw.clone()).unwrap();
        assert_eq!(a.encrypted_credentials.as_deref(), Some("blob=="));
        assert_eq!(serde_json::to_value(&a).unwrap(), raw);
    }
}
