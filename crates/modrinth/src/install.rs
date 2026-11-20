//! Installing projects and their required dependencies into an instance.

use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::time::{SystemTime, UNIX_EPOCH};

use mango_archive::safe_archive_path;
use mango_core::{Loader, ModRecord, ModType, Paths, Profile};
use mango_net::{download_file, DownloadOptions, Expected};

use crate::client::{pick_version, primary_file, Client};
use crate::deps::{direct_dependency_refs, has_exact_installed_version, pid};
use crate::error::{Error, Result};
use crate::types::{Project, Version, VersionFile};

pub type LogFn = dyn Fn(&str) + Send + Sync;

pub fn loader_name(l: Loader) -> &'static str {
    match l {
        Loader::Vanilla => "vanilla",
        Loader::Fabric => "fabric",
        Loader::Quilt => "quilt",
        Loader::Forge => "forge",
        Loader::Neoforge => "neoforge",
    }
}

/// Map a Modrinth `project_type` to the stored type (unknown means mod).
pub fn mod_type_of(project_type: &str) -> ModType {
    match project_type {
        "shader" => ModType::Shader,
        "resourcepack" => ModType::Resourcepack,
        "datapack" => ModType::Datapack,
        _ => ModType::Mod,
    }
}

pub fn mods_dir(paths: &Paths, profile_id: &str) -> Result<PathBuf> {
    target_dir_for(paths, ModType::Mod, profile_id)
}

/// Instance folder a content type lives in. The profile id must be a UUID.
pub fn target_dir_for(paths: &Paths, kind: ModType, profile_id: &str) -> Result<PathBuf> {
    let base = paths.instance_dir(profile_id)?;
    Ok(base.join(match kind {
        ModType::Mod => "mods",
        ModType::Shader => "shaderpacks",
        ModType::Resourcepack => "resourcepacks",
        ModType::Datapack => "datapacks",
    }))
}

/// Filenames from the API must be a bare basename.
pub(crate) fn safe_filename(name: &str) -> Result<&str> {
    let bare = !name.is_empty()
        && !name.contains(['/', '\\', '\0'])
        && name != "."
        && name != ".."
        && Path::new(name).file_name().and_then(|n| n.to_str()) == Some(name);
    if bare {
        Ok(name)
    } else {
        Err(Error::InvalidFilename)
    }
}

pub(crate) fn expected_for(file: &VersionFile) -> Expected {
    Expected {
        sha512: file.hashes.sha512.clone(),
        sha1: file.hashes.sha1.clone(),
        size: file.size,
        ..Expected::default()
    }
}

pub(crate) fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Walk a mod's required dependencies depth first. Optional dependencies are
/// skipped; a dependency that cannot be resolved is skipped too.
fn resolve_dependencies<'a>(
    client: &'a Client,
    version: &'a Version,
    loader: &'a str,
    game_version: &'a str,
    seen: &'a mut HashSet<String>,
    out: &'a mut Vec<Version>,
) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
    Box::pin(async move {
        for dep in &version.dependencies {
            if dep.dependency_type != "required" {
                continue;
            }
            let Some(key) = dep.project_id.clone().or_else(|| dep.version_id.clone()) else {
                continue;
            };
            if !seen.insert(key) {
                continue;
            }
            let dep_version = if let Some(vid) = &dep.version_id {
                client.version(vid).await.ok()
            } else if let Some(p) = &dep.project_id {
                let versions = client
                    .versions(p, Some(loader), Some(game_version))
                    .await
                    .unwrap_or_default();
                pick_version(&versions).cloned()
            } else {
                None
            };
            let Some(dep_version) = dep_version else {
                continue;
            };
            out.push(dep_version.clone());
            resolve_dependencies(client, &dep_version, loader, game_version, seen, out).await;
        }
    })
}

/// Install a project into a profile, pulling required dependencies with it.
/// Returns the new records (the caller merges them, see [`merge_installed`]).
pub async fn install_project(
    client: &Client,
    paths: &Paths,
    profile: &Profile,
    project_id_or_slug: &str,
    version_id: Option<&str>,
    with_dependencies: bool,
    on_log: &LogFn,
) -> Result<Vec<ModRecord>> {
    let project = client.project(project_id_or_slug).await?;
    let is_mod = project.project_type == "mod";
    let loader = loader_name(profile.loader);

    let version = match version_id {
        Some(v) => client.version(v).await?,
        None => {
            let versions = client
                .versions(
                    &project.id,
                    is_mod.then_some(loader),
                    Some(&profile.mc_version),
                )
                .await?;
            pick_version(&versions)
                .cloned()
                .ok_or_else(|| Error::NoCompatibleVersion {
                    title: project.title.clone(),
                    loader: loader.to_string(),
                    mc_version: profile.mc_version.clone(),
                })?
        }
    };

    let mut to_install: Vec<(Project, Version)> = vec![(project.clone(), version.clone())];
    if with_dependencies && is_mod {
        let mut deps = Vec::new();
        resolve_dependencies(
            client,
            &version,
            loader,
            &profile.mc_version,
            &mut HashSet::new(),
            &mut deps,
        )
        .await;
        if !deps.is_empty() {
            let ids: Vec<String> = deps.iter().map(|d| d.project_id.clone()).collect();
            let by_id: HashMap<String, Project> = client
                .projects(&ids)
                .await?
                .into_iter()
                .map(|p| (p.id.clone(), p))
                .collect();
            on_log(&format!(
                "Also installing {} required dependenc{}",
                deps.len(),
                if deps.len() == 1 { "y" } else { "ies" }
            ));
            for d in deps {
                let p = by_id
                    .get(&d.project_id)
                    .cloned()
                    .unwrap_or_else(|| Project {
                        id: d.project_id.clone(),
                        slug: String::new(),
                        title: d.name.clone(),
                        project_type: "mod".into(),
                        icon_url: None,
                        extra: Default::default(),
                    });
                to_install.push((p, d));
            }
        }
    }

    let selected_versions: HashMap<String, String> = to_install
        .iter()
        .map(|(p, v)| (p.id.clone(), v.id.clone()))
        .collect();
    let selected_projects: HashMap<String, String> = to_install
        .iter()
        .map(|(p, v)| (v.id.clone(), p.id.clone()))
        .collect();

    let mut installed = Vec::new();
    for (p, v) in &to_install {
        let is_dependency = p.id != project.id;
        if is_dependency && has_exact_installed_version(&profile.mods, &p.id, &v.id) {
            continue;
        }
        let Some(file) = primary_file(v) else {
            continue;
        };
        let kind = mod_type_of(&p.project_type);
        let dir = target_dir_for(paths, kind, &profile.id)?;
        let filename = safe_filename(&file.filename)?;
        let dest = safe_archive_path(&dir, filename)?;
        on_log(&format!("Downloading {} {}…", p.title, v.version_number));
        download_file(
            &file.url,
            &dest,
            &DownloadOptions {
                expected: expected_for(file),
                ..DownloadOptions::default()
            },
        )
        .await?;
        installed.push(ModRecord {
            project_id: Some(p.id.clone()),
            slug: Some(p.slug.clone()),
            title: Some(p.title.clone()),
            kind: Some(kind),
            icon: p.icon_url.clone(),
            version_id: Some(v.id.clone()),
            version_number: Some(v.version_number.clone()),
            filename: Some(filename.to_string()),
            file: Some(dest.to_string_lossy().into_owned()),
            game_versions: Some(v.game_versions.clone()),
            loaders: Some(v.loaders.clone()),
            installed_at: Some(now_ms()),
            dependency: Some(is_dependency),
            required_dependencies: Some(direct_dependency_refs(
                v,
                &selected_versions,
                &selected_projects,
            )),
            ..ModRecord::default()
        });
    }
    Ok(installed)
}

/// Merge freshly installed records into a mod list. A replaced project's old
/// jar is deleted and its enabled state is kept; new ones start enabled.
pub async fn merge_installed(mods: &[ModRecord], installed: Vec<ModRecord>) -> Vec<ModRecord> {
    let mut out = mods.to_vec();
    for mut rec in installed {
        if let Some(i) = out.iter().position(|m| pid(m) == pid(&rec)) {
            if out[i].file != rec.file {
                if let Some(old) = &out[i].file {
                    let _ = tokio::fs::remove_file(old).await;
                }
            }
            rec.enabled = Some(out[i].enabled != Some(false));
            out[i] = rec;
        } else {
            rec.enabled = Some(true);
            out.push(rec);
        }
    }
    out
}
