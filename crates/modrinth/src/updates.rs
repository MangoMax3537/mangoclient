//! Update checks that honour dependency version pins.

use std::collections::HashMap;
use std::sync::Arc;

use mango_core::{ModType, Profile};
use mango_net::pool;
use serde::Serialize;

use crate::client::{pick_version, Client};
use crate::deps::{hydrate_dependency_metadata, pid};
use crate::error::Result;
use crate::install::loader_name;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RequiredBy {
    pub project_id: String,
    pub title: String,
    pub version_id: String,
}

/// Either an available update or (with `conflict`) a project pinned to
/// several different versions by different parents.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub project_id: String,
    #[serde(rename = "type")]
    pub kind: ModType,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compatibility: Option<bool>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub conflict: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required_by: Vec<RequiredBy>,
}

/// Check every Modrinth-managed content item for a compatible newer build.
pub async fn check_updates(client: &Client, profile: &Profile) -> Result<Vec<UpdateInfo>> {
    let mods = hydrate_dependency_metadata(client, &profile.mods)
        .await?
        .mods;
    let content: Vec<_> = mods
        .iter()
        .filter(|r| {
            r.local != Some(true)
                && !pid(r).is_empty()
                && matches!(
                    r.kind.unwrap_or(ModType::Mod),
                    ModType::Mod | ModType::Resourcepack | ModType::Shader
                )
        })
        .cloned()
        .collect();

    let mut pins: HashMap<String, Vec<String>> = HashMap::new();
    let mut required_by: HashMap<String, Vec<RequiredBy>> = HashMap::new();
    for parent in &mods {
        if parent.enabled == Some(false) {
            continue;
        }
        for d in parent.required_dependencies.iter().flatten() {
            let Some(vid) = &d.version_id else { continue };
            let list = pins.entry(d.project_id.clone()).or_default();
            if !list.contains(vid) {
                list.push(vid.clone());
            }
            required_by
                .entry(d.project_id.clone())
                .or_default()
                .push(RequiredBy {
                    project_id: pid(parent).to_string(),
                    title: parent.title.clone().unwrap_or_default(),
                    version_id: vid.clone(),
                });
        }
    }
    let pins = Arc::new(pins);
    let required_by = Arc::new(required_by);
    let loader = loader_name(profile.loader).to_string();
    let mc = profile.mc_version.clone();
    let client = client.clone();

    let results = pool(content, 6, move |item, _| {
        let (client, pins, required_by, loader, mc) = (
            client.clone(),
            pins.clone(),
            required_by.clone(),
            loader.clone(),
            mc.clone(),
        );
        async move {
            let kind = item.kind.unwrap_or(ModType::Mod);
            let project_id = pid(&item).to_string();
            let title = item.title.clone().unwrap_or_default();
            let pinned = pins.get(&project_id);
            let (latest, compatibility) = match pinned.map(Vec::len) {
                Some(1) => {
                    let v = client.version(&pinned?[0]).await.ok()?;
                    let differs = Some(&v.id) != item.version_id.as_ref();
                    (v, differs)
                }
                Some(n) if n > 1 => {
                    return Some(UpdateInfo {
                        project_id: project_id.clone(),
                        kind,
                        title,
                        from: None,
                        to: None,
                        version_id: None,
                        compatibility: None,
                        conflict: true,
                        required_by: required_by.get(&project_id).cloned().unwrap_or_default(),
                    });
                }
                _ => {
                    let versions = client
                        .versions(
                            &project_id,
                            (kind == ModType::Mod).then_some(loader.as_str()),
                            Some(&mc),
                        )
                        .await
                        .ok()?;
                    (pick_version(&versions)?.clone(), false)
                }
            };
            if Some(&latest.id) == item.version_id.as_ref() {
                return None;
            }
            Some(UpdateInfo {
                project_id,
                kind,
                title,
                from: item.version_number.clone(),
                to: Some(latest.version_number.clone()),
                version_id: Some(latest.id.clone()),
                compatibility: Some(compatibility),
                conflict: false,
                required_by: Vec::new(),
            })
        }
    })
    .await?;
    Ok(results.into_iter().flatten().collect())
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileUpdates {
    pub profile_id: String,
    pub updates: Vec<UpdateInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Check each profile in turn; one failing profile does not stop the rest.
pub async fn check_all_updates(client: &Client, profiles: &[Profile]) -> Vec<ProfileUpdates> {
    let mut out = Vec::new();
    for p in profiles {
        let (updates, error) = match check_updates(client, p).await {
            Ok(u) => (u, None),
            Err(e) => (Vec::new(), Some(e.to_string())),
        };
        out.push(ProfileUpdates {
            profile_id: p.id.clone(),
            updates,
            error,
        });
    }
    out
}
