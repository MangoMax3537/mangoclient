//! TypeScript-only descriptions of loosely typed wire values.
//!
//! Some inputs and outputs travel as plain strings, maps or `serde_json::Value`
//! on the Rust side (the core validates them at runtime). These types are never
//! constructed; they exist so the generated bindings state the exact contract.
//! `tests` below keep them in step with the Rust validators.

use mango_core::Loader;
use serde::Serialize;
use ts_rs::TS;

/// A content type or `modpack`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(rename = "ProjectType")]
pub enum ProjectKind {
    Mod,
    Shader,
    Resourcepack,
    Datapack,
    Modpack,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "lowercase")]
pub enum SearchSort {
    Relevance,
    Downloads,
    Follows,
    Newest,
    Updated,
}

/// The three visible faces of a block icon (`mangoimg://block/<id>/<face>`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct BlockFaces {
    pub top: String,
    pub left: String,
    pub right: String,
}

/// `config_set` patch: every field optional, unknown keys are rejected by the
/// core. `curseforgeApiKey` is write-only (never returned).
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ConfigPatch {
    #[ts(as = "Option<crate::wire::Language>", optional)]
    pub language: Option<String>,
    #[ts(optional)]
    pub ram: Option<u32>,
    #[ts(optional)]
    pub java_path: Option<String>,
    #[ts(optional)]
    pub java_args: Option<String>,
    #[ts(optional)]
    pub fullscreen: Option<bool>,
    #[ts(optional)]
    pub width: Option<u32>,
    #[ts(optional)]
    pub height: Option<u32>,
    #[ts(optional)]
    pub keep_launcher_open: Option<bool>,
    #[ts(optional)]
    pub hide_on_launch: Option<bool>,
    #[ts(optional)]
    pub show_snapshots: Option<bool>,
    #[ts(optional)]
    pub mango_config: Option<bool>,
    #[ts(optional)]
    pub performance_mods: Option<bool>,
    #[ts(optional)]
    pub concurrent_downloads: Option<u32>,
    #[ts(as = "Option<crate::wire::PerformancePreset>", optional)]
    pub performance_preset: Option<String>,
    #[ts(optional = nullable)]
    pub selected_profile: Option<String>,
    #[ts(optional = nullable)]
    pub selected_account: Option<String>,
    #[ts(optional)]
    pub first_run_done: Option<bool>,
    #[ts(optional)]
    pub telemetry: Option<bool>,
    #[ts(optional)]
    pub sidebar_open: Option<bool>,
    /// Write-only; `null` or `""` clears it.
    #[ts(optional = nullable)]
    pub curseforge_api_key: Option<String>,
    #[ts(optional)]
    pub blocked_mods_modrinth_fallback: Option<bool>,
    /// Absolute paths of existing folders.
    #[ts(optional)]
    pub blocked_mods_watch_dirs: Option<Vec<String>>,
    #[ts(optional)]
    pub blocked_mods_check_subfolders: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    De,
    En,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "lowercase")]
pub enum PerformancePreset {
    Potato,
    Balanced,
    Quality,
}

/// `profile_update` patch: only these fields are applied, derived view fields
/// are ignored by the service.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ProfilePatch {
    #[ts(optional)]
    pub name: Option<String>,
    #[ts(optional)]
    pub mc_version: Option<String>,
    #[ts(optional)]
    pub loader: Option<Loader>,
    #[ts(optional = nullable)]
    pub loader_version: Option<String>,
    #[ts(optional = nullable)]
    pub color: Option<String>,
    #[ts(optional = nullable)]
    pub ram: Option<u32>,
    #[ts(optional)]
    pub java_args: Option<String>,
    #[ts(optional = nullable)]
    pub mango_config: Option<bool>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};
    use std::collections::BTreeSet;

    /// All keys of a serialized all-`Some` instance.
    fn keys(v: Value) -> BTreeSet<String> {
        v.as_object()
            .map(|o| o.keys().cloned().collect())
            .unwrap_or_default()
    }

    #[test]
    fn config_patch_keys_are_exactly_the_keys_the_core_accepts() {
        let patch = ConfigPatch {
            language: Some("en".into()),
            ram: Some(0),
            java_path: Some(String::new()),
            java_args: Some(String::new()),
            fullscreen: Some(false),
            width: Some(0),
            height: Some(0),
            keep_launcher_open: Some(false),
            hide_on_launch: Some(false),
            show_snapshots: Some(false),
            mango_config: Some(false),
            performance_mods: Some(false),
            concurrent_downloads: Some(0),
            performance_preset: Some("balanced".into()),
            selected_profile: Some(String::new()),
            selected_account: Some(String::new()),
            first_run_done: Some(false),
            telemetry: Some(false),
            sidebar_open: Some(false),
            curseforge_api_key: Some(String::new()),
            blocked_mods_modrinth_fallback: Some(false),
            blocked_mods_watch_dirs: Some(vec![]),
            blocked_mods_check_subfolders: Some(false),
        };
        let ours = keys(serde_json::to_value(patch).unwrap_or(Value::Null));
        // `theme` is accepted by the core but not part of the UI contract.
        let accepted: BTreeSet<String> = ours
            .iter()
            .filter(|k| mango_core::validation::config_field(k, &json!(null)).is_some())
            .cloned()
            .collect();
        assert_eq!(ours, accepted, "a ConfigPatch key is unknown to the core");
        for k in [
            "language",
            "ram",
            "javaPath",
            "javaArgs",
            "fullscreen",
            "width",
            "height",
            "keepLauncherOpen",
            "hideOnLaunch",
            "showSnapshots",
            "mangoConfig",
            "performanceMods",
            "concurrentDownloads",
            "performancePreset",
            "selectedProfile",
            "selectedAccount",
            "firstRunDone",
            "telemetry",
            "sidebarOpen",
            "curseforgeApiKey",
            "blockedModsModrinthFallback",
            "blockedModsWatchDirs",
            "blockedModsCheckSubfolders",
        ] {
            assert!(ours.contains(k), "missing {k}");
        }
        assert_eq!(ours.len(), 23);
    }

    #[test]
    fn profile_patch_keys_are_the_ones_the_service_applies() {
        let patch = ProfilePatch {
            name: Some("n".into()),
            mc_version: Some("1.21".into()),
            loader: Some(Loader::Fabric),
            loader_version: Some("1".into()),
            color: Some("#aabbcc".into()),
            ram: Some(2048),
            java_args: Some(String::new()),
            mango_config: Some(true),
        };
        let ours = keys(serde_json::to_value(patch).unwrap_or(Value::Null));
        let svc_keys: BTreeSet<String> = [
            "name",
            "mcVersion",
            "loader",
            "loaderVersion",
            "color",
            "ram",
            "javaArgs",
            "mangoConfig",
        ]
        .into_iter()
        .map(String::from)
        .collect();
        assert_eq!(ours, svc_keys);
        for k in &ours {
            assert!(mango_core::validation::profile_field(k, &json!(null)).is_some());
        }
    }
}
