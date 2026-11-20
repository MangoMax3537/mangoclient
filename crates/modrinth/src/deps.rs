//! Dependency graph bookkeeping, enable/disable ordering and uninstall.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use mango_core::{ModDependency, ModRecord, ModType};
use mango_net::pool;
use serde::Serialize;

use crate::client::Client;
use crate::error::{Error, Result};
use crate::types::Version;

pub(crate) fn pid(r: &ModRecord) -> &str {
    r.project_id.as_deref().unwrap_or("")
}

fn title(r: &ModRecord) -> String {
    r.title.clone().unwrap_or_else(|| pid(r).to_string())
}

/// A dependency is reusable only when it is the exact Modrinth build selected
/// by the parent mod. Sharing a project id alone is not enough.
pub fn has_exact_installed_version(mods: &[ModRecord], project_id: &str, version_id: &str) -> bool {
    mods.iter()
        .any(|r| pid(r) == project_id && r.version_id.as_deref() == Some(version_id))
}

pub(crate) fn direct_dependency_refs(
    version: &Version,
    versions_by_project: &HashMap<String, String>,
    project_by_version: &HashMap<String, String>,
) -> Vec<ModDependency> {
    let mut refs = Vec::new();
    let mut seen = HashSet::new();
    for d in &version.dependencies {
        if d.dependency_type != "required" {
            continue;
        }
        let project_id = d.project_id.clone().or_else(|| {
            d.version_id
                .as_ref()
                .and_then(|v| project_by_version.get(v).cloned())
        });
        let Some(project_id) = project_id.filter(|p| !p.is_empty()) else {
            continue;
        };
        if !seen.insert(project_id.clone()) {
            continue;
        }
        let version_id = d
            .version_id
            .clone()
            .or_else(|| versions_by_project.get(&project_id).cloned());
        refs.push(ModDependency {
            project_id,
            version_id,
        });
    }
    refs
}

#[derive(Debug, Clone)]
pub struct Hydrated {
    pub mods: Vec<ModRecord>,
    pub changed: bool,
    /// False when some record's metadata could not be fetched.
    pub complete: bool,
}

fn is_mod_kind(r: &ModRecord) -> bool {
    matches!(r.kind, None | Some(ModType::Mod))
}

/// Fill the dependency graph on records stored before MangoClient kept it.
/// Failures leave the record untouched and mark the result incomplete.
pub async fn hydrate_dependency_metadata(client: &Client, mods: &[ModRecord]) -> Result<Hydrated> {
    let missing: Vec<usize> = mods
        .iter()
        .enumerate()
        .filter(|(_, r)| {
            is_mod_kind(r)
                && r.local != Some(true)
                && r.version_id.is_some()
                && r.required_dependencies.is_none()
        })
        .map(|(i, _)| i)
        .collect();
    if missing.is_empty() {
        return Ok(Hydrated {
            mods: mods.to_vec(),
            changed: false,
            complete: true,
        });
    }

    let fetch = |client: &Client, ids: Vec<String>| {
        let client = client.clone();
        async move {
            pool(ids, 6, move |id, _| {
                let client = client.clone();
                async move { client.version(&id).await.ok() }
            })
            .await
        }
    };
    let ids: Vec<String> = missing
        .iter()
        .filter_map(|&i| mods[i].version_id.clone())
        .collect();
    let versions = fetch(client, ids).await?;

    let mut versions_by_project = HashMap::new();
    let mut project_by_version = HashMap::new();
    for r in mods {
        if let (Some(p), Some(v)) = (&r.project_id, &r.version_id) {
            versions_by_project.insert(p.clone(), v.clone());
            project_by_version.insert(v.clone(), p.clone());
        }
    }

    let mut unresolved: Vec<String> = Vec::new();
    for v in versions.iter().flatten() {
        for d in &v.dependencies {
            if d.dependency_type == "required" && d.project_id.is_none() {
                if let Some(vid) = &d.version_id {
                    if !project_by_version.contains_key(vid) && !unresolved.contains(vid) {
                        unresolved.push(vid.clone());
                    }
                }
            }
        }
    }
    let owners = fetch(client, unresolved.clone()).await?;
    for (vid, owner) in unresolved.into_iter().zip(owners) {
        if let Some(o) = owner.filter(|o| !o.project_id.is_empty()) {
            project_by_version.insert(vid, o.project_id);
        }
    }

    let mut out = mods.to_vec();
    let mut complete = true;
    let mut changed = false;
    for (&i, version) in missing.iter().zip(&versions) {
        match version {
            None => complete = false,
            Some(v) => {
                changed = true;
                out[i].required_dependencies = Some(direct_dependency_refs(
                    v,
                    &versions_by_project,
                    &project_by_version,
                ));
            }
        }
    }
    Ok(Hydrated {
        mods: out,
        changed,
        complete,
    })
}

fn dependency_ids(r: &ModRecord) -> impl Iterator<Item = &str> {
    r.required_dependencies
        .iter()
        .flatten()
        .map(|d| d.project_id.as_str())
}

/// Installed records that require `project_id`.
pub fn installed_dependents<'a>(mods: &'a [ModRecord], project_id: &str) -> Vec<&'a ModRecord> {
    mods.iter()
        .filter(|r| dependency_ids(r).any(|d| d == project_id))
        .collect()
}

/// Toggle by renaming: loaders ignore files ending in `.disabled`. Returns the
/// path the record should now point at, or `None` when the record is unknown.
pub async fn set_mod_enabled(
    mods: &[ModRecord],
    project_id: &str,
    enabled: bool,
) -> Result<Option<String>> {
    let Some(record) = mods.iter().find(|r| pid(r) == project_id) else {
        return Ok(None);
    };
    let file = record.file.clone().unwrap_or_default();
    let disabled = if file.ends_with(".disabled") {
        file.clone()
    } else {
        format!("{file}.disabled")
    };
    let plain = file.strip_suffix(".disabled").unwrap_or(&file).to_string();
    let (from, to) = if enabled {
        (disabled, plain)
    } else {
        (plain, disabled)
    };
    if from != to && tokio::fs::try_exists(&from).await.unwrap_or(false) {
        tokio::fs::rename(&from, &to)
            .await
            .map_err(|e| Error::io(Path::new(&from), e))?;
    }
    Ok(Some(to))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Affected {
    pub project_id: String,
    pub title: String,
}

#[derive(Debug, Clone)]
pub struct ToggleOutcome {
    pub mods: Vec<ModRecord>,
    pub affected: Vec<Affected>,
}

fn visit_requirements(
    id: &str,
    by_id: &HashMap<&str, &ModRecord>,
    visited: &mut HashSet<String>,
    order: &mut Vec<String>,
) -> Result<()> {
    if !visited.insert(id.to_string()) {
        return Ok(());
    }
    let record = by_id
        .get(id)
        .ok_or_else(|| Error::DependencyNotInstalled(id.to_string()))?;
    for d in record.required_dependencies.iter().flatten() {
        let installed = by_id
            .get(d.project_id.as_str())
            .ok_or_else(|| Error::DependencyNotInstalled(d.project_id.clone()))?;
        if d.version_id.is_some() && installed.version_id != d.version_id {
            return Err(Error::DependencyVersionMismatch {
                title: title(record),
                dependency: title(installed),
            });
        }
        visit_requirements(&d.project_id, by_id, visited, order)?;
    }
    order.push(id.to_string());
    Ok(())
}

fn visit_dependents(
    id: &str,
    mods: &[ModRecord],
    visited: &mut HashSet<String>,
    order: &mut Vec<String>,
) {
    if !visited.insert(id.to_string()) {
        return;
    }
    for d in installed_dependents(mods, id) {
        visit_dependents(pid(d), mods, visited, order);
    }
    order.push(id.to_string());
}

/// Toggle a complete dependency tree. Requirements are enabled before their
/// consumer; dependents are disabled before the dependency they consume.
pub async fn set_enabled_with_dependencies(
    mods: &[ModRecord],
    project_id: &str,
    enabled: bool,
) -> Result<ToggleOutcome> {
    let by_id: HashMap<&str, &ModRecord> = mods.iter().map(|r| (pid(r), r)).collect();
    if !by_id.contains_key(project_id) {
        return Ok(ToggleOutcome {
            mods: mods.to_vec(),
            affected: Vec::new(),
        });
    }
    let mut order = Vec::new();
    let mut visited = HashSet::new();
    if enabled {
        visit_requirements(project_id, &by_id, &mut visited, &mut order)?;
    } else {
        visit_dependents(project_id, mods, &mut visited, &mut order);
    }

    let mut out = mods.to_vec();
    let mut affected = Vec::new();
    for id in order {
        let Some(i) = out.iter().position(|r| pid(r) == id) else {
            continue;
        };
        if (out[i].enabled != Some(false)) == enabled {
            continue;
        }
        let new_path = set_mod_enabled(&out, &id, enabled).await?;
        out[i].enabled = Some(enabled);
        if new_path.is_some() {
            out[i].file = new_path;
        }
        affected.push(Affected {
            project_id: id,
            title: title(&out[i]),
        });
    }
    Ok(ToggleOutcome {
        mods: out,
        affected,
    })
}

/// The `modrinth:toggle` flow: backfill dependency metadata, refuse when it
/// is incomplete, then toggle the tree. The returned mods include the
/// backfilled metadata; the caller persists them.
pub async fn toggle(
    client: &Client,
    mods: &[ModRecord],
    project_id: &str,
    enabled: bool,
) -> Result<ToggleOutcome> {
    let hydrated = hydrate_dependency_metadata(client, mods).await?;
    if !hydrated.complete {
        return Err(Error::MetadataIncomplete);
    }
    set_enabled_with_dependencies(&hydrated.mods, project_id, enabled).await
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Dependent {
    pub project_id: String,
    pub title: String,
}

#[derive(Debug, Clone)]
pub enum UninstallOutcome {
    /// Other installed mods require the project; nothing was changed.
    Blocked {
        dependents: Vec<Dependent>,
        mods: Vec<ModRecord>,
    },
    Removed {
        mods: Vec<ModRecord>,
    },
}

/// Remove a project's file and record unless other mods depend on it.
pub async fn uninstall(
    client: &Client,
    mods: &[ModRecord],
    project_id: &str,
) -> Result<UninstallOutcome> {
    let mods = hydrate_dependency_metadata(client, mods).await?.mods;
    let dependents: Vec<Dependent> = installed_dependents(&mods, project_id)
        .into_iter()
        .map(|r| Dependent {
            project_id: pid(r).to_string(),
            title: title(r),
        })
        .collect();
    if !dependents.is_empty() {
        return Ok(UninstallOutcome::Blocked { dependents, mods });
    }
    if let Some(file) = mods
        .iter()
        .find(|r| pid(r) == project_id)
        .and_then(|r| r.file.as_deref())
    {
        // A file that is already gone is fine.
        let _ = tokio::fs::remove_file(file).await;
    }
    let mods = mods.into_iter().filter(|r| pid(r) != project_id).collect();
    Ok(UninstallOutcome::Removed { mods })
}
