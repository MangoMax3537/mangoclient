//! The content source: Modrinth.
//!
//! Everything the install planner needs is expressed in the normalized
//! [`ProjectInfo`] / [`VersionInfo`]; [`ModrinthSource`] maps the Modrinth API
//! onto them.

use crate::content::{hit_from_search, loader_accepts, public_icon, valid_project_ref};
use crate::error::{AppError, Res};
use crate::models::{ContentType, ProjectDetail, ProjectHit, SearchParams, SearchResult};
use mango_core::{Loader, ModType};
use mango_modrinth as modrinth;
use mango_net::{Expected, FetchOptions};
use serde_json::Value;
use std::collections::HashMap;

/// Project ids -> ids of the profiles that have them.
pub(crate) type InstalledIndex = HashMap<String, Vec<String>>;

const SORTS: [&str; 5] = ["relevance", "downloads", "follows", "newest", "updated"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    Content(ContentType),
    Modpack,
}

#[derive(Debug, Clone)]
pub(crate) struct ProjectInfo {
    pub id: String,
    pub slug: String,
    pub title: String,
    /// `None` = a kind of project the launcher cannot install.
    pub kind: Option<Kind>,
    /// Raw remote icon URL (turned into `mangoimg://` on the way out).
    pub icon: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Release {
    Stable,
    Beta,
    Alpha,
}

impl Release {
    pub fn as_str(self) -> &'static str {
        match self {
            Release::Stable => "release",
            Release::Beta => "beta",
            Release::Alpha => "alpha",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Relation {
    Required,
    Optional,
    Incompatible,
}

#[derive(Debug, Clone)]
pub(crate) struct DepRef {
    pub project_id: String,
    /// A pinned build.
    pub version_id: Option<String>,
    pub relation: Relation,
}

#[derive(Debug, Clone)]
pub(crate) struct VersionInfo {
    pub project_id: String,
    pub id: String,
    pub number: String,
    pub name: String,
    pub release: Release,
    pub game_versions: Vec<String>,
    /// Lower-case loader names.
    pub loaders: Vec<String>,
    /// Sortable ISO timestamp.
    pub date: String,
    pub downloads: u64,
    pub size: Option<u64>,
    pub filename: String,
    pub expected: Expected,
    pub deps: Vec<DepRef>,
}

/// Does a build run on `mc` and (when the content needs one) `loader`?
pub(crate) fn fits(v: &VersionInfo, mc: &str, loader: Option<&str>) -> bool {
    v.game_versions.iter().any(|g| g == mc) && loader.is_none_or(|l| loader_accepts(l, &v.loaders))
}

/// The best build for an instance: a build that names the loader itself beats
/// an accepted fallback (Fabric build on Quilt), then release over beta over
/// alpha, then the newest.
pub(crate) fn pick_best<'a>(
    versions: &'a [VersionInfo],
    mc: &str,
    loader: Option<&str>,
) -> Option<&'a VersionInfo> {
    let fitting: Vec<&VersionInfo> = versions.iter().filter(|v| fits(v, mc, loader)).collect();
    let pool = match loader {
        Some(l) => {
            let exact: Vec<&VersionInfo> = fitting
                .iter()
                .copied()
                .filter(|v| v.loaders.iter().any(|x| x == l))
                .collect();
            if exact.is_empty() {
                fitting
            } else {
                exact
            }
        }
        None => fitting,
    };
    pool.into_iter()
        .max_by(|a, b| b.release.cmp(&a.release).then_with(|| a.date.cmp(&b.date)))
}

/// The newest, most stable build regardless of instance (modpacks).
pub(crate) fn pick_latest(versions: &[VersionInfo]) -> Option<&VersionInfo> {
    versions
        .iter()
        .max_by(|a, b| b.release.cmp(&a.release).then_with(|| a.date.cmp(&b.date)))
}

/// Which loader constrains builds of `kind` on a profile with `loader`:
/// `Err(())` = cannot work at all (mods on a vanilla instance), `Ok(None)` =
/// the loader does not matter (shaders, packs).
#[allow(clippy::result_unit_err)]
pub(crate) fn loader_need(kind: ContentType, loader: Loader) -> Result<Option<&'static str>, ()> {
    match (kind, loader) {
        (ModType::Mod, Loader::Vanilla) => Err(()),
        (ModType::Mod, l) => Ok(Some(modrinth::loader_name(l))),
        _ => Ok(None),
    }
}

// ---------------------------------------------------------------- Modrinth

pub(crate) struct ModrinthSource {
    pub client: modrinth::Client,
    pub fetch: FetchOptions,
}

fn mr_kind(project_type: &str) -> Option<Kind> {
    Some(match project_type {
        "mod" => Kind::Content(ModType::Mod),
        "shader" => Kind::Content(ModType::Shader),
        "resourcepack" => Kind::Content(ModType::Resourcepack),
        "datapack" => Kind::Content(ModType::Datapack),
        "modpack" => Kind::Modpack,
        _ => return None,
    })
}

fn mr_version(v: &modrinth::Version) -> VersionInfo {
    let file = modrinth::primary_file(v);
    let deps = v
        .dependencies
        .iter()
        .filter_map(|d| {
            let relation = match d.dependency_type.as_str() {
                "required" => Relation::Required,
                "optional" => Relation::Optional,
                "incompatible" => Relation::Incompatible,
                _ => return None,
            };
            let project_id = d.project_id.clone().unwrap_or_default();
            let version_id = d.version_id.clone().filter(|p| !p.is_empty());
            // A pin may stand in for the project id; one of them is needed.
            (!project_id.is_empty() || version_id.is_some()).then_some(DepRef {
                project_id,
                version_id,
                relation,
            })
        })
        .collect();
    VersionInfo {
        project_id: v.project_id.clone(),
        id: v.id.clone(),
        number: v.version_number.clone(),
        name: v.name.clone(),
        release: match v.version_type.as_str() {
            "beta" => Release::Beta,
            "alpha" => Release::Alpha,
            _ => Release::Stable,
        },
        game_versions: v.game_versions.clone(),
        loaders: v.loaders.clone(),
        date: v.date_published.clone(),
        downloads: v
            .extra
            .get("downloads")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        size: file.and_then(|f| f.size),
        filename: file.map(|f| f.filename.clone()).unwrap_or_default(),
        expected: file.map_or_else(Expected::default, |f| Expected {
            sha512: f.hashes.sha512.clone(),
            sha1: f.hashes.sha1.clone(),
            size: f.size,
            ..Expected::default()
        }),
        deps,
    }
}

impl ModrinthSource {
    /// Owner's name (`/project/<id>/members`); empty when unavailable.
    async fn author(&self, project_id: &str) -> String {
        let url = format!("{}/project/{project_id}/members", self.client.base());
        let members: Vec<Value> = mango_net::get_json(&url, &self.fetch)
            .await
            .unwrap_or_default();
        let name = |m: &Value| {
            m.pointer("/user/username")
                .and_then(Value::as_str)
                .map(str::to_string)
        };
        members
            .iter()
            .find(|m| m.get("role").and_then(Value::as_str) == Some("Owner"))
            .and_then(name)
            .or_else(|| members.first().and_then(name))
            .unwrap_or_default()
    }
}

impl ModrinthSource {
    pub(crate) async fn search(
        &self,
        params: &SearchParams,
        installed: &InstalledIndex,
    ) -> Res<SearchResult> {
        let loader = params.loader.clone().filter(|l| !l.is_empty());
        if let Some(l) = &loader {
            if mango_loaders::Loader::parse(l).is_none() {
                return Err(AppError::validation("Invalid loader"));
            }
        }
        let query = modrinth::SearchParams {
            query: params.query.chars().take(200).collect(),
            // No loader means no loader facet ("vanilla").
            loader: loader.unwrap_or_else(|| "vanilla".into()),
            game_version: params.mc_version.clone().unwrap_or_default(),
            project_type: params.project_type.clone(),
            categories: params
                .categories
                .clone()
                .unwrap_or_default()
                .into_iter()
                .filter(|c| valid_project_ref(c))
                .take(16)
                .collect(),
            limit: params.limit.clamp(1, 100),
            offset: params.offset,
            index: params.sort.clone(),
            open_source_only: false,
        };
        let raw = self.client.search(&query).await?;
        let hits = raw
            .get("hits")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|h| hit_from_search(h, installed))
                    .collect()
            })
            .unwrap_or_default();
        Ok(SearchResult {
            hits,
            total: raw.get("total_hits").and_then(Value::as_u64).unwrap_or(0),
            offset: raw.get("offset").and_then(Value::as_u64).unwrap_or(0),
        })
    }

    pub(crate) async fn detail(&self, id: &str, installed: &InstalledIndex) -> Res<ProjectDetail> {
        if !valid_project_ref(id) {
            return Err(AppError::validation("Invalid project id"));
        }
        let project = self.client.project(id).await?;
        let x = &project.extra;
        let str_of = |k: &str| x.get(k).and_then(Value::as_str).map(str::to_string);
        let strings = |k: &str| -> Vec<String> {
            x.get(k)
                .and_then(Value::as_array)
                .map(|a| {
                    a.iter()
                        .filter_map(Value::as_str)
                        .map(str::to_string)
                        .collect()
                })
                .unwrap_or_default()
        };
        let author = self.author(&project.id).await;
        let hit = ProjectHit {
            project_id: project.id.clone(),
            slug: project.slug.clone(),
            title: project.title.clone(),
            author,
            description: str_of("description").unwrap_or_default(),
            kind: project.project_type.clone(),
            icon_url: project.icon_url.as_deref().and_then(public_icon),
            downloads: x.get("downloads").and_then(Value::as_u64).unwrap_or(0),
            follows: x.get("followers").and_then(Value::as_u64).unwrap_or(0),
            categories: strings("categories"),
            game_versions: strings("game_versions"),
            loaders: strings("loaders"),
            date_modified: str_of("updated").unwrap_or_default(),
            installed_in: installed.get(&project.id).cloned().unwrap_or_default(),
        };
        let gallery = x
            .get("gallery")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|g| g.get("url").and_then(Value::as_str))
                    .filter_map(public_icon)
                    .collect()
            })
            .unwrap_or_default();
        let license = x.get("license").and_then(|l| {
            l.get("name")
                .or_else(|| l.get("id"))
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
        });
        Ok(ProjectDetail {
            hit,
            body: str_of("body").unwrap_or_default(),
            gallery,
            license,
            source_url: str_of("source_url")
                .filter(|u| mango_mango::security::is_allowed_external_url(u)),
            issues_url: str_of("issues_url")
                .filter(|u| mango_mango::security::is_allowed_external_url(u)),
        })
    }

    pub(crate) async fn project(&self, id: &str) -> Res<ProjectInfo> {
        if !valid_project_ref(id) {
            return Err(AppError::validation("Invalid project id"));
        }
        let p = self.client.project(id).await?;
        Ok(ProjectInfo {
            kind: mr_kind(&p.project_type),
            icon: p.icon_url.clone(),
            id: p.id,
            slug: p.slug,
            title: p.title,
        })
    }

    pub(crate) async fn versions(
        &self,
        id: &str,
        mc: Option<&str>,
        loader: Option<&str>,
    ) -> Res<Vec<VersionInfo>> {
        if !valid_project_ref(id) {
            return Err(AppError::validation("Invalid project id"));
        }
        Ok(self
            .client
            .versions(id, loader, mc)
            .await?
            .iter()
            .map(mr_version)
            .collect())
    }

    pub(crate) async fn version(&self, _project_id: &str, version_id: &str) -> Res<VersionInfo> {
        if !valid_project_ref(version_id) {
            return Err(AppError::validation("Invalid version id"));
        }
        Ok(mr_version(&self.client.version(version_id).await?))
    }

    pub(crate) async fn download_url(
        &self,
        _project: &ProjectInfo,
        v: &VersionInfo,
    ) -> Res<Option<String>> {
        // The URL is part of the version document; re-read it from the
        // (cached) version so `VersionInfo` stays free of URLs.
        let full = self.client.version(&v.id).await?;
        Ok(modrinth::primary_file(&full)
            .map(|f| f.url.clone())
            .filter(|u| !u.is_empty()))
    }
}

/// The sort orders Modrinth understands.
pub(crate) fn check_sort(sort: &str) -> Res<()> {
    if SORTS.contains(&sort) {
        Ok(())
    } else {
        Err(AppError::validation("Unknown sort order"))
    }
}
