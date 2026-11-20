//! `content_*` and `modrinth_*` commands.
//!
//! The modrinth crate never persists; every mutation here reads the profile's
//! mod list, hands it to the crate and writes the result back through the
//! store.

use crate::error::{AppError, Res};
use crate::events::{Event, InstallProgressEvent};
use crate::image_protocol::remote_url;
use crate::models::{
    ContentItem, ProjectDetail, ProjectHit, ProjectVersion, SearchParams, SearchResult,
    ToggleResult, UninstallResult, UpdateInfo,
};
use crate::service::{lock, AppService, Inner};
use mango_core::{validate_profile_id, ModRecord, ModType, Profile};
use mango_modrinth as modrinth;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

const PROJECT_TYPES: [&str; 5] = ["mod", "shader", "resourcepack", "datapack", "modpack"];
const SORTS: [&str; 5] = ["relevance", "downloads", "follows", "newest", "updated"];
const HIT_LOADERS: [&str; 9] = [
    "fabric",
    "quilt",
    "forge",
    "neoforge",
    "liteloader",
    "rift",
    "iris",
    "optifine",
    "vanilla",
];

/// Modrinth ids and slugs: `[A-Za-z0-9_-]{1,64}`.
fn valid_project_ref(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 64
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

/// The remote icon, a bundled-cache file or nothing (legacy inline `data:`).
pub(crate) fn public_icon(icon: &str) -> Option<String> {
    if icon.starts_with("mangoimg://") {
        Some(icon.to_string())
    } else if icon.starts_with("https://") || icon.starts_with("http://") {
        Some(remote_url(icon))
    } else {
        None
    }
}

fn record_pid(r: &ModRecord) -> &str {
    r.project_id.as_deref().unwrap_or_default()
}

fn to_item(rec: &ModRecord, all: &[ModRecord], newer: Option<&String>) -> ContentItem {
    let pid = record_pid(rec);
    let filename = rec
        .filename
        .clone()
        .filter(|f| !f.is_empty())
        .or_else(|| {
            rec.file
                .as_deref()
                .and_then(|f| Path::new(f).file_name())
                .map(|n| mango_content::jar_name(&n.to_string_lossy()))
        })
        .unwrap_or_default();
    ContentItem {
        project_id: pid.to_string(),
        slug: rec.slug.clone().filter(|s| !s.is_empty()),
        title: rec
            .title
            .clone()
            .filter(|t| !t.is_empty())
            .unwrap_or_else(|| {
                if filename.is_empty() {
                    pid.to_string()
                } else {
                    filename.clone()
                }
            }),
        kind: rec.kind.unwrap_or(ModType::Mod),
        version_id: rec.version_id.clone(),
        version_number: rec.version_number.clone(),
        filename,
        enabled: rec.enabled != Some(false),
        local: rec.local == Some(true),
        icon_url: rec.icon.as_deref().and_then(public_icon),
        required_by: modrinth::installed_dependents(all, pid)
            .into_iter()
            .map(|d| record_pid(d).to_string())
            .collect(),
        dependency: rec.dependency == Some(true),
        update_available: newer.cloned(),
        installed_at: rec.installed_at,
    }
}

impl AppService {
    pub(crate) fn profile(&self, id: &str) -> Res<Profile> {
        validate_profile_id(id)?;
        self.inner
            .with_store(|s| s.get_profile(id).cloned())
            .ok_or_else(AppError::profile_not_found)
    }

    fn save_mods(&self, id: &str, mods: &[ModRecord]) -> Res<()> {
        self.inner
            .with_store(|s| s.update_profile(id, &json!({ "mods": mods })).map(|_| ()))?;
        Ok(())
    }

    fn items(&self, profile_id: &str, mods: &[ModRecord]) -> Vec<ContentItem> {
        let updates = lock(&self.inner.updates)
            .get(profile_id)
            .cloned()
            .unwrap_or_default();
        mods.iter()
            .map(|r| to_item(r, mods, updates.get(record_pid(r))))
            .collect()
    }

    /// The profile's content; first reconciles `mods/` with the stored list.
    pub async fn content_list(&self, profile_id: String) -> Res<Vec<ContentItem>> {
        let profile = self.profile(&profile_id)?;
        let paths = self.inner.paths.clone();
        let snapshot = profile.clone();
        let synced = tokio::task::spawn_blocking(move || {
            mango_content::sync_profile_mods(&paths, &snapshot)
        })
        .await??;
        let mods = if synced.changed {
            self.save_mods(&profile_id, &synced.mods)?;
            synced.mods
        } else {
            profile.mods
        };
        Ok(self.items(&profile_id, &mods))
    }

    pub async fn content_toggle(
        &self,
        profile_id: String,
        project_id: String,
        enabled: bool,
    ) -> Res<ToggleResult> {
        let profile = self.profile(&profile_id)?;
        let mods = profile.mods;
        let rec = mods
            .iter()
            .find(|r| record_pid(r) == project_id)
            .ok_or_else(content_not_found)?;
        let (mods, affected) = if rec.local == Some(true) {
            let next = mango_content::set_local_mod_enabled(rec, enabled)?;
            let mut out = mods.clone();
            if let Some(slot) = out.iter_mut().find(|r| record_pid(r) == project_id) {
                *slot = next;
            }
            (out, vec![project_id.clone()])
        } else {
            let outcome =
                modrinth::toggle(&self.inner.modrinth, &mods, &project_id, enabled).await?;
            (
                outcome.mods,
                outcome.affected.into_iter().map(|a| a.project_id).collect(),
            )
        };
        self.save_mods(&profile_id, &mods)?;
        Ok(ToggleResult {
            items: self.items(&profile_id, &mods),
            affected,
        })
    }

    pub async fn content_uninstall(
        &self,
        profile_id: String,
        project_id: String,
    ) -> Res<UninstallResult> {
        let profile = self.profile(&profile_id)?;
        if !profile.mods.iter().any(|r| record_pid(r) == project_id) {
            return Err(content_not_found());
        }
        let outcome = modrinth::uninstall(&self.inner.modrinth, &profile.mods, &project_id).await?;
        let (blocked, dependents, mods) = match outcome {
            modrinth::UninstallOutcome::Blocked { dependents, mods } => (
                true,
                dependents.into_iter().map(|d| d.project_id).collect(),
                mods,
            ),
            modrinth::UninstallOutcome::Removed { mods } => (false, Vec::new(), mods),
        };
        if mods != profile.mods {
            self.save_mods(&profile_id, &mods)?;
        }
        lock(&self.inner.updates)
            .entry(profile_id.clone())
            .and_modify(|m| {
                m.remove(&project_id);
            });
        Ok(UninstallResult {
            blocked,
            dependents,
            items: self.items(&profile_id, &mods),
        })
    }

    pub async fn content_check_updates(&self, profile_id: String) -> Res<Vec<UpdateInfo>> {
        let mut profile = self.profile(&profile_id)?;
        // Backfill dependency metadata once and keep it, so later checks and
        // toggles do not have to ask Modrinth again.
        let hydrated =
            modrinth::hydrate_dependency_metadata(&self.inner.modrinth, &profile.mods).await?;
        if hydrated.changed {
            self.save_mods(&profile_id, &hydrated.mods)?;
            profile.mods = hydrated.mods;
        }
        let found = modrinth::check_updates(&self.inner.modrinth, &profile).await?;
        let mut newer = HashMap::new();
        let mut out = Vec::new();
        for u in found {
            if !u.conflict {
                if let Some(to) = &u.to {
                    newer.insert(u.project_id.clone(), to.clone());
                }
            }
            out.push(UpdateInfo {
                project_id: u.project_id,
                title: u.title,
                kind: u.kind,
                from: u.from,
                to: u.to.unwrap_or_default(),
                version_id: u.version_id.unwrap_or_default(),
                compatibility: u.compatibility.unwrap_or(true),
                conflict: u.conflict,
                required_by: u.required_by.into_iter().map(|r| r.project_id).collect(),
            });
        }
        lock(&self.inner.updates).insert(profile_id, newer);
        Ok(out)
    }

    pub fn content_open_folder(&self, profile_id: String, kind: ModType) -> Res<String> {
        let dir = modrinth::target_dir_for(&self.inner.paths, kind, &profile_id)?;
        std::fs::create_dir_all(&dir)?;
        self.open_dir(&dir);
        Ok(dir.display().to_string())
    }

    // ---- Modrinth browsing ----

    pub async fn modrinth_search(&self, params: SearchParams) -> Res<SearchResult> {
        if !PROJECT_TYPES.contains(&params.project_type.as_str()) {
            return Err(AppError::validation("Unknown project type"));
        }
        if !SORTS.contains(&params.sort.as_str()) {
            return Err(AppError::validation("Unknown sort order"));
        }
        let loader = params.loader.filter(|l| !l.is_empty());
        if let Some(l) = &loader {
            if mango_loaders::Loader::parse(l).is_none() {
                return Err(AppError::validation("Invalid loader"));
            }
        }
        let query = modrinth::SearchParams {
            query: params.query.chars().take(200).collect(),
            // No loader means no loader facet ("vanilla").
            loader: loader.unwrap_or_else(|| "vanilla".into()),
            game_version: params.mc_version.unwrap_or_default(),
            project_type: params.project_type,
            categories: params
                .categories
                .unwrap_or_default()
                .into_iter()
                .filter(|c| valid_project_ref(c))
                .take(16)
                .collect(),
            limit: params.limit.clamp(1, 100),
            offset: params.offset,
            index: params.sort,
            open_source_only: false,
        };
        let raw = self.inner.modrinth.search(&query).await?;
        let installed = self.installed_index();
        let hits = raw
            .get("hits")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|h| hit_from_search(h, &installed))
                    .collect()
            })
            .unwrap_or_default();
        Ok(SearchResult {
            hits,
            total: raw.get("total_hits").and_then(Value::as_u64).unwrap_or(0),
            offset: raw.get("offset").and_then(Value::as_u64).unwrap_or(0),
        })
    }

    /// project id -> profile ids that have it installed.
    fn installed_index(&self) -> HashMap<String, Vec<String>> {
        self.inner.with_store(|s| {
            let mut index: HashMap<String, Vec<String>> = HashMap::new();
            for p in &s.profiles {
                for m in &p.mods {
                    if let Some(id) = m.project_id.as_deref().filter(|i| !i.starts_with("local:")) {
                        index.entry(id.to_string()).or_default().push(p.id.clone());
                    }
                }
            }
            index
        })
    }

    pub async fn modrinth_project(&self, project_id: String) -> Res<ProjectDetail> {
        if !valid_project_ref(&project_id) {
            return Err(AppError::validation("Invalid project id"));
        }
        let project = self.inner.modrinth.project(&project_id).await?;
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
        let author = self.project_author(&project.id).await;
        let installed = self.installed_index();
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

    /// Owner's name (`/project/<id>/members`); empty when unavailable. The
    /// crate has no members call, so this uses the shared HTTP layer.
    async fn project_author(&self, project_id: &str) -> String {
        let url = format!(
            "{}/project/{project_id}/members",
            self.inner.modrinth.base()
        );
        let members: Vec<Value> = mango_net::get_json(&url, &self.inner.opts.fetch)
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

    pub async fn modrinth_versions(
        &self,
        project_id: String,
        profile_id: Option<String>,
    ) -> Res<Vec<ProjectVersion>> {
        if !valid_project_ref(&project_id) {
            return Err(AppError::validation("Invalid project id"));
        }
        let profile = match profile_id {
            Some(id) => Some(self.profile(&id)?),
            None => None,
        };
        let versions = self
            .inner
            .modrinth
            .versions(&project_id, None, None)
            .await?;
        // Loaders only matter for mods; shaders and packs match on the game version.
        let loader_matters = match &profile {
            Some(_) => {
                let kind = self.inner.modrinth.project(&project_id).await?.project_type;
                kind == "mod"
            }
            None => false,
        };
        let mut out: Vec<ProjectVersion> = versions
            .into_iter()
            .map(|v| {
                let compatible = profile.as_ref().is_none_or(|p| {
                    let mc = v.game_versions.contains(&p.mc_version);
                    let loader = !loader_matters
                        || p.loader == mango_core::Loader::Vanilla
                        || loader_accepts(modrinth::loader_name(p.loader), &v.loaders);
                    mc && loader
                });
                ProjectVersion {
                    version_id: v.id,
                    version_number: v.version_number,
                    name: v.name,
                    kind: match v.version_type.as_str() {
                        "beta" => "beta",
                        "alpha" => "alpha",
                        _ => "release",
                    }
                    .to_string(),
                    game_versions: v.game_versions,
                    loaders: v.loaders,
                    date_published: v.date_published,
                    downloads: v
                        .extra
                        .get("downloads")
                        .and_then(Value::as_u64)
                        .unwrap_or(0),
                    compatible,
                }
            })
            .collect();
        // Compatible builds first, newest first inside each group.
        out.sort_by(|a, b| {
            b.compatible
                .cmp(&a.compatible)
                .then_with(|| b.date_published.cmp(&a.date_published))
        });
        Ok(out)
    }

    pub async fn modrinth_install(
        &self,
        profile_id: String,
        project_id: String,
        version_id: Option<String>,
    ) -> Res<Vec<ContentItem>> {
        let profile = self.profile(&profile_id)?;
        if !valid_project_ref(&project_id) {
            return Err(AppError::validation("Invalid project id"));
        }
        if version_id.as_deref().is_some_and(|v| !valid_project_ref(v)) {
            return Err(AppError::validation("Invalid version id"));
        }
        let project = self.inner.modrinth.project(&project_id).await?;
        if project.project_type == "modpack" {
            return Err(AppError::new(
                "use_modpack_install",
                "Modpacks are installed as a new profile",
            ));
        }

        let progress = InstallReporter::new(&self.inner, &profile_id, &project.id);
        progress.emit(0, 1, &project.title);
        let log = {
            let progress = progress.clone();
            move |line: &str| progress.emit(0, 1, line)
        };
        let installed = modrinth::install_project(
            &self.inner.modrinth,
            &self.inner.paths,
            &profile,
            &project.id,
            version_id.as_deref(),
            true,
            &log,
        )
        .await?;
        // Merge into the list as it is now, not as it was when we started.
        let current = self.profile(&profile_id)?.mods;
        let touched: Vec<String> = installed
            .iter()
            .map(|r| record_pid(r).to_string())
            .collect();
        let merged = modrinth::merge_installed(&current, installed).await;
        self.save_mods(&profile_id, &merged)?;
        lock(&self.inner.updates)
            .entry(profile_id.clone())
            .and_modify(|m| m.retain(|k, _| !touched.contains(k)));
        progress.emit(1, 1, "done");
        Ok(self.items(&profile_id, &merged))
    }

    pub async fn modrinth_install_modpack(
        &self,
        project_id: String,
        version_id: Option<String>,
        name: Option<String>,
    ) -> Res<crate::types::ProfileView> {
        if !valid_project_ref(&project_id) {
            return Err(AppError::validation("Invalid project id"));
        }
        if version_id.as_deref().is_some_and(|v| !valid_project_ref(v)) {
            return Err(AppError::validation("Invalid version id"));
        }
        let client = &self.inner.modrinth;
        let project = client.project(&project_id).await?;
        if project.project_type != "modpack" {
            return Err(AppError::new(
                "not_a_modpack",
                "This project is not a modpack",
            ));
        }
        let version = match version_id {
            Some(v) => client.version(&v).await?,
            None => {
                let all = client.versions(&project.id, None, None).await?;
                modrinth::pick_version(&all).cloned().ok_or_else(|| {
                    AppError::new("no_compatible_version", "The modpack has no versions")
                })?
            }
        };

        // Provisional values; the pack's index says what it really needs.
        let provisional_loader = version
            .loaders
            .iter()
            .find(|l| mango_loaders::Loader::parse(l).is_some())
            .cloned()
            .unwrap_or_else(|| "vanilla".into());
        let provisional_mc = version
            .game_versions
            .first()
            .cloned()
            .unwrap_or_else(|| "1.21.11".into());
        let name: String = name
            .map(|n| n.trim().to_string())
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| project.title.clone())
            .chars()
            .take(128)
            .collect();
        let created = self.inner.with_store(|s| {
            s.add_profile(&json!({
                "name": name,
                "mcVersion": provisional_mc,
                "loader": provisional_loader,
            }))
            .or_else(|_| {
                s.add_profile(&json!({"name": name, "mcVersion": "1.21.11", "loader": "vanilla"}))
            })
        })?;

        match self
            .finish_modpack(&created, &project.id, &version.id)
            .await
        {
            Ok(view) => Ok(view),
            Err(e) => {
                // A half-installed pack is worse than none.
                let _ = self.inner.with_store(|s| s.delete_profile(&created.id));
                Err(e)
            }
        }
    }

    async fn finish_modpack(
        &self,
        profile: &Profile,
        project_id: &str,
        version_id: &str,
    ) -> Res<crate::types::ProfileView> {
        let reporter = InstallReporter::new(&self.inner, &profile.id, project_id);
        reporter.emit(0, 1, "modpack");
        let log = {
            let r = reporter.clone();
            move |line: &str| r.emit(0, 1, line)
        };
        let progress: Arc<modrinth::ProgressFn> = {
            let r = reporter.clone();
            Arc::new(move |p: &modrinth::ModpackProgress| {
                r.emit_throttled(p.done as u64, p.total as u64, &p.label);
            })
        };
        let info = modrinth::install_modpack(
            &self.inner.modrinth,
            &self.inner.paths,
            profile,
            version_id,
            &log,
            Some(progress),
        )
        .await?;

        let mut patch = json!({"loader": info.loader, "loaderVersion": info.loader_version});
        if let Some(mc) = info.mc_version.filter(|m| !m.is_empty()) {
            patch["mcVersion"] = Value::String(mc);
        }
        let updated = self
            .inner
            .with_store(|s| s.update_profile(&profile.id, &patch).map(|p| p.cloned()))?
            .ok_or_else(AppError::profile_not_found)?;

        // The pack's jars become ordinary (local) content records.
        let paths = self.inner.paths.clone();
        let snapshot = updated.clone();
        let synced = tokio::task::spawn_blocking(move || {
            mango_content::sync_profile_mods(&paths, &snapshot)
        })
        .await??;
        self.save_mods(&profile.id, &synced.mods)?;
        reporter.emit(1, 1, "done");
        let fresh = self.profile(&profile.id)?;
        Ok(crate::types::ProfileView::from(&fresh))
    }
}

fn content_not_found() -> AppError {
    AppError::new("content_not_found", "That content is not installed")
}

/// Does a version built for `loaders` run on `loader`? Quilt also loads Fabric
/// mods, NeoForge also loads (older) Forge ones.
fn loader_accepts(loader: &str, loaders: &[String]) -> bool {
    let has = |l: &str| loaders.iter().any(|x| x == l);
    match loader {
        "quilt" => has("quilt") || has("fabric"),
        "neoforge" => has("neoforge") || has("forge"),
        other => has(other),
    }
}

fn hit_from_search(h: &Value, installed: &HashMap<String, Vec<String>>) -> Option<ProjectHit> {
    let s = |k: &str| {
        h.get(k)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    };
    let n = |k: &str| h.get(k).and_then(Value::as_u64).unwrap_or(0);
    let strings = |k: &str| -> Vec<String> {
        h.get(k)
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default()
    };
    let project_id = s("project_id");
    if project_id.is_empty() {
        return None;
    }
    let categories = strings("categories");
    let loaders = categories
        .iter()
        .filter(|c| HIT_LOADERS.contains(&c.as_str()))
        .cloned()
        .collect();
    let kind = match s("project_type").as_str() {
        "shader" => "shader",
        "resourcepack" => "resourcepack",
        "datapack" => "datapack",
        "modpack" => "modpack",
        _ => "mod",
    };
    Some(ProjectHit {
        installed_in: installed.get(&project_id).cloned().unwrap_or_default(),
        project_id,
        slug: s("slug"),
        title: s("title"),
        author: s("author"),
        description: s("description"),
        kind: kind.to_string(),
        icon_url: h
            .get("icon_url")
            .and_then(Value::as_str)
            .and_then(public_icon),
        downloads: n("downloads"),
        follows: n("follows"),
        categories,
        game_versions: strings("versions"),
        loaders,
        date_modified: s("date_modified"),
    })
}

/// Emits `install:progress` for one install operation.
#[derive(Clone)]
struct InstallReporter {
    inner: Arc<Inner>,
    profile_id: String,
    project_id: String,
}

impl InstallReporter {
    fn new(inner: &Arc<Inner>, profile_id: &str, project_id: &str) -> Self {
        InstallReporter {
            inner: Arc::clone(inner),
            profile_id: profile_id.to_string(),
            project_id: project_id.to_string(),
        }
    }

    fn send(&self, done: u64, total: u64, label: &str) {
        self.inner
            .emit(Event::InstallProgress(InstallProgressEvent {
                profile_id: self.profile_id.clone(),
                project_id: self.project_id.clone(),
                done,
                total,
                label: label.to_string(),
            }));
    }

    fn emit(&self, done: u64, total: u64, label: &str) {
        self.send(done, total, label);
    }

    /// At most 10 per second; the last one (done == total) always passes.
    fn emit_throttled(&self, done: u64, total: u64, label: &str) {
        let key = format!("install:{}", self.profile_id);
        let due = lock(&self.inner.throttle).ready(&key, done >= total, Instant::now());
        if due {
            self.send(done, total, label);
        }
    }
}
