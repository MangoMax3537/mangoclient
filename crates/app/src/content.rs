//! `content_*` and `modrinth_*` commands.
//!
//! The modrinth crate never persists; every mutation here reads the profile's
//! mod list, hands it to the crate and writes the result back through the
//! store.

use crate::error::{AppError, Res};
use crate::events::{Event, InstallProgressEvent};
use crate::image_protocol::remote_url;
use crate::models::{
    ContentItem, ProjectDetail, ProjectHit, ProjectVersion, Provider, SearchParams, SearchResult,
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

pub(crate) const PROJECT_TYPES: [&str; 5] =
    ["mod", "shader", "resourcepack", "datapack", "modpack"];
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
pub(crate) fn valid_project_ref(s: &str) -> bool {
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
        provider: Provider::of_record(rec),
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

    pub(crate) fn save_mods(&self, id: &str, mods: &[ModRecord]) -> Res<()> {
        self.inner
            .with_store(|s| s.update_profile(id, &json!({ "mods": mods })).map(|_| ()))?;
        Ok(())
    }

    pub(crate) fn items(&self, profile_id: &str, mods: &[ModRecord]) -> Vec<ContentItem> {
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
        let mods = self.contain_records(&profile_id, &profile.mods);
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
            let outcome = modrinth::toggle(
                &self.inner.modrinth,
                &shield_foreign(&mods),
                &project_id,
                enabled,
            )
            .await?;
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
        let title = profile
            .mods
            .iter()
            .find(|r| record_pid(r) == project_id)
            .and_then(|r| r.title.clone())
            .unwrap_or_else(|| project_id.clone());
        self.auto_snapshot(
            &profile_id,
            crate::backups::BackupReason::BeforeDelete,
            format!("Before removing {title}"),
        )
        .await;
        let outcome = modrinth::uninstall(
            &self.inner.modrinth,
            &shield_foreign(&self.contain_records(&profile_id, &profile.mods)),
            &project_id,
        )
        .await?;
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
        // Modrinth's checker only ever sees Modrinth records (it would ask
        // Modrinth about CurseForge file ids otherwise).
        let mut mr_profile = profile.clone();
        mr_profile.mods = profile
            .mods
            .iter()
            .filter(|r| !is_curseforge(r))
            .cloned()
            .collect();
        // Backfill dependency metadata once and keep it, so later checks and
        // toggles do not have to ask Modrinth again.
        let hydrated =
            modrinth::hydrate_dependency_metadata(&self.inner.modrinth, &mr_profile.mods).await?;
        if hydrated.changed {
            let mut fresh = hydrated.mods.iter();
            for rec in profile.mods.iter_mut().filter(|r| !is_curseforge(r)) {
                if let Some(h) = fresh.next() {
                    *rec = h.clone();
                }
            }
            self.save_mods(&profile_id, &profile.mods)?;
            mr_profile.mods = hydrated.mods;
        }
        let found = modrinth::check_updates(&self.inner.modrinth, &mr_profile).await?;
        let mut newer = HashMap::new();
        let mut out = Vec::new();
        for u in found {
            if !u.conflict {
                if let Some(to) = &u.to {
                    newer.insert(u.project_id.clone(), to.clone());
                }
            }
            out.push(UpdateInfo {
                provider: Some(Provider::Modrinth),
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
        for u in self.check_curseforge_updates(&profile).await {
            newer.insert(u.project_id.clone(), u.to.clone());
            out.push(u);
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

    // ---- Modrinth browsing (deprecated aliases of the provider commands) ----

    /// Deprecated alias of [`AppService::search_projects`].
    pub async fn modrinth_search(&self, params: SearchParams) -> Res<SearchResult> {
        self.search_projects(SearchParams {
            provider: Provider::Modrinth,
            ..params
        })
        .await
    }

    /// Deprecated alias of [`AppService::project_detail`].
    pub async fn modrinth_project(&self, project_id: String) -> Res<ProjectDetail> {
        self.project_detail(Provider::Modrinth, project_id).await
    }

    /// Deprecated alias of [`AppService::project_versions`].
    pub async fn modrinth_versions(
        &self,
        project_id: String,
        profile_id: Option<String>,
    ) -> Res<Vec<ProjectVersion>> {
        self.project_versions(Provider::Modrinth, project_id, profile_id)
            .await
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

        if profile.mods.iter().any(|r| record_pid(r) == project.id) {
            self.auto_snapshot(
                &profile_id,
                crate::backups::BackupReason::BeforeUpdate,
                format!("Before updating {}", project.title),
            )
            .await;
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
        let current = self.contain_records(&profile_id, &self.profile(&profile_id)?.mods);
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
            Ok(view) => {
                // The pack's logo becomes the instance icon (best effort).
                self.apply_pack_logo(&created.id, project.icon_url.as_deref())
                    .await;
                Ok(self
                    .profile(&created.id)
                    .map_or(view, |p| crate::blocked::view_of(&self.inner.paths, &p)))
            }
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
        self.finish_modrinth_info(profile, &reporter, info).await
    }

    /// What follows a Modrinth pack install, whether the pack came from the
    /// API or a file: the pack's own loader and version win over the
    /// provisional values, and its jars become ordinary (local) records.
    pub(crate) async fn finish_modrinth_info(
        &self,
        profile: &Profile,
        reporter: &InstallReporter,
        info: modrinth::ModpackInfo,
    ) -> Res<crate::types::ProfileView> {
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
        Ok(crate::blocked::view_of(&self.inner.paths, &fresh))
    }
}

pub(crate) fn is_curseforge(r: &ModRecord) -> bool {
    r.provider.as_deref() == Some("curseforge")
}

/// A copy of the list in which CurseForge records carry an (empty) dependency
/// list when they have none stored, so Modrinth's metadata backfill never asks
/// Modrinth about CurseForge ids.
impl AppService {
    /// Withhold the `file` of every record that no longer passes the store's
    /// safety check (outside the instance, a symlink, behind a symlinked
    /// folder), so the removal and rename code never touches such a path.
    pub(crate) fn contain_records(&self, profile_id: &str, mods: &[ModRecord]) -> Vec<ModRecord> {
        mods.iter()
            .cloned()
            .map(|mut r| {
                if r.file.is_some()
                    && !mango_core::mod_record_is_safe(&self.inner.paths, profile_id, &r)
                {
                    tracing::warn!("ignoring an unsafe content path of {:?}", r.title);
                    r.file = None;
                }
                r
            })
            .collect()
    }
}

pub(crate) fn shield_foreign(mods: &[ModRecord]) -> Vec<ModRecord> {
    mods.iter()
        .cloned()
        .map(|mut r| {
            if is_curseforge(&r) && r.required_dependencies.is_none() {
                r.required_dependencies = Some(Vec::new());
            }
            r
        })
        .collect()
}

fn content_not_found() -> AppError {
    AppError::new("content_not_found", "That content is not installed")
}

/// Does a version built for `loaders` run on `loader`? Quilt also loads Fabric
/// mods, NeoForge also loads (older) Forge ones.
pub(crate) fn loader_accepts(loader: &str, loaders: &[String]) -> bool {
    let has = |l: &str| loaders.iter().any(|x| x == l);
    match loader {
        "quilt" => has("quilt") || has("fabric"),
        "neoforge" => has("neoforge") || has("forge"),
        other => has(other),
    }
}

pub(crate) fn hit_from_search(
    h: &Value,
    installed: &HashMap<String, Vec<String>>,
) -> Option<ProjectHit> {
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
        provider: Provider::Modrinth,
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
pub(crate) struct InstallReporter {
    inner: Arc<Inner>,
    profile_id: String,
    project_id: String,
}

impl InstallReporter {
    pub(crate) fn new(inner: &Arc<Inner>, profile_id: &str, project_id: &str) -> Self {
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

    pub(crate) fn emit(&self, done: u64, total: u64, label: &str) {
        self.send(done, total, label);
    }

    /// At most 10 per second; the last one (done == total) always passes.
    pub(crate) fn emit_throttled(&self, done: u64, total: u64, label: &str) {
        let key = format!("install:{}", self.profile_id);
        let due = lock(&self.inner.throttle).ready(&key, done >= total, Instant::now());
        if due {
            self.send(done, total, label);
        }
    }
}
