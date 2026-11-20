//! Part 4 of `docs/ipc.md`: browsing, install planning, modpacks (Modrinth).
//!
//! Browsing and planning go through `source.rs`; persistence goes through the
//! store exactly like the legacy Modrinth path.

use crate::content::{has_foreign_provider, InstallReporter, PROJECT_TYPES};
use crate::error::{AppError, Res};
use crate::events::{Event, LaunchLogEvent, LogLevelName};
use crate::models::{
    ContentItem, ContentType, InstallPlan, InstallPlanItem, InstallTarget, ModpackPlan,
    PlanConflict, PlanRole, ProjectDetail, ProjectVersion, SearchParams, SearchResult,
    TargetReason, TargetState,
};
use crate::service::{lock, now_ms, AppService};
use crate::source::{
    check_sort, fits, loader_need, pick_best, pick_latest, InstalledIndex, Kind, ModrinthSource,
    ProjectInfo, Relation, VersionInfo,
};
use crate::types::ProfileView;
use mango_core::{ModRecord, ModType, Profile};
use mango_modrinth as modrinth;
use mango_net::{download_file, DownloadOptions};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};

/// Upper bound on a dependency crawl.
const MAX_PLAN_ITEMS: usize = 100;

fn use_modpack_install() -> AppError {
    AppError::new(
        "use_modpack_install",
        "Modpacks are installed as a new profile",
    )
}

fn not_a_modpack() -> AppError {
    AppError::new("not_a_modpack", "This project is not a modpack")
}

fn unsupported_content() -> AppError {
    AppError::new(
        "unsupported_content",
        "This kind of project cannot be installed",
    )
}

fn no_compatible(title: &str, profile: &Profile) -> AppError {
    AppError::new(
        "no_compatible_version",
        format!(
            "{title} has no build for {} {}",
            modrinth::loader_name(profile.loader),
            profile.mc_version
        ),
    )
}

/// Filenames from an API must be one bare basename.
pub(crate) fn safe_name(name: &str) -> Res<&str> {
    let bare = !name.is_empty()
        && !name.contains(['/', '\\', '\0'])
        && name != "."
        && name != ".."
        && Path::new(name).file_name().and_then(|n| n.to_str()) == Some(name);
    if bare {
        Ok(name)
    } else {
        Err(AppError::new(
            "invalid_filename",
            "The provider returned an invalid filename",
        ))
    }
}

fn safe_dest(dir: &Path, name: &str) -> Res<PathBuf> {
    mango_archive::safe_archive_path(dir, safe_name(name)?)
        .map_err(|e| AppError::new("unsafe_path", e.to_string()))
}

/// One build chosen for the plan, with everything `install_apply` needs.
#[derive(Debug, Clone)]
struct Pick {
    project: ProjectInfo,
    version: VersionInfo,
    item: InstallPlanItem,
}

struct Built {
    plan: InstallPlan,
    /// Non-incompatible items in plan order; `[0]` is the requested one.
    picks: Vec<Pick>,
}

fn role_rank(r: PlanRole) -> u8 {
    match r {
        PlanRole::Requested => 0,
        PlanRole::Required => 1,
        PlanRole::Optional => 2,
        PlanRole::Incompatible => 3,
    }
}

/// The Modrinth record of a project. Local jars and records of sources this
/// version no longer knows never match.
fn installed_record<'a>(profile: &'a Profile, project_id: &str) -> Option<&'a ModRecord> {
    profile.mods.iter().find(|r| {
        r.local != Some(true)
            && !has_foreign_provider(r)
            && r.project_id.as_deref() == Some(project_id)
    })
}

fn loader_label(profile: &Profile) -> &'static str {
    modrinth::loader_name(profile.loader)
}

impl AppService {
    // ---- source ----

    pub(crate) fn source(&self) -> ModrinthSource {
        ModrinthSource {
            client: self.inner.modrinth.clone(),
            fetch: self.inner.opts.fetch.clone(),
        }
    }

    /// Project id -> profiles that have it from Modrinth.
    fn installed_index(&self) -> InstalledIndex {
        self.inner.with_store(|s| {
            let mut index: InstalledIndex = HashMap::new();
            for p in &s.profiles {
                for m in &p.mods {
                    if m.local == Some(true) || has_foreign_provider(m) {
                        continue;
                    }
                    if let Some(id) = m.project_id.as_deref().filter(|i| !i.starts_with("local:")) {
                        index.entry(id.to_string()).or_default().push(p.id.clone());
                    }
                }
            }
            index
        })
    }

    pub(crate) fn warn(&self, profile_id: &str, line: String) {
        self.inner.emit(Event::LaunchLog(LaunchLogEvent {
            profile_id: profile_id.to_string(),
            line,
            level: LogLevelName::Warn,
        }));
    }

    // ---- commands: browsing ----

    pub async fn search_projects(&self, params: SearchParams) -> Res<SearchResult> {
        if !PROJECT_TYPES.contains(&params.project_type.as_str()) {
            return Err(AppError::validation("Unknown project type"));
        }
        check_sort(&params.sort)?;
        let src = self.source();
        let installed = self.installed_index();
        src.search(&params, &installed).await
    }

    pub async fn project_detail(&self, project_id: String) -> Res<ProjectDetail> {
        let src = self.source();
        let installed = self.installed_index();
        src.detail(&project_id, &installed).await
    }

    pub async fn project_versions(
        &self,
        project_id: String,
        profile_id: Option<String>,
    ) -> Res<Vec<ProjectVersion>> {
        let profile = match profile_id {
            Some(id) => Some(self.profile(&id)?),
            None => None,
        };
        let src = self.source();
        // Loaders only matter for mods; shaders and packs match on the game version.
        let kind = match &profile {
            Some(_) => match src.project(&project_id).await?.kind {
                Some(Kind::Content(k)) => Some(k),
                _ => None,
            },
            None => None,
        };
        let versions = src.versions(&project_id, None, None).await?;
        let mut out: Vec<ProjectVersion> = versions
            .into_iter()
            .map(|v| {
                let compatible = profile.as_ref().is_none_or(|p| {
                    match loader_need(kind.unwrap_or(ModType::Shader), p.loader) {
                        Ok(need) => fits(&v, &p.mc_version, need),
                        Err(()) => false,
                    }
                });
                ProjectVersion {
                    version_id: v.id,
                    version_number: v.number,
                    name: v.name,
                    kind: v.release.as_str().to_string(),
                    game_versions: v.game_versions,
                    loaders: v.loaders,
                    date_published: v.date,
                    downloads: v.downloads,
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

    // ---- planning ----

    /// Resolve a dependency to a concrete build for the profile. A dependency
    /// that cannot be resolved is skipped, as the legacy installer did.
    async fn resolve_dep(
        &self,
        src: &ModrinthSource,
        project_id: &str,
        version_id: Option<&str>,
        profile: &Profile,
    ) -> Option<(ProjectInfo, VersionInfo)> {
        let mc = profile.mc_version.as_str();
        let version = match version_id {
            Some(v) => src.version(project_id, v).await.ok()?,
            None => {
                let project = src.project(project_id).await.ok()?;
                let Some(Kind::Content(kind)) = project.kind else {
                    return None;
                };
                let need = loader_need(kind, profile.loader).ok()?;
                let all = src.versions(project_id, Some(mc), need).await.ok()?;
                pick_best(&all, mc, need)?.clone()
            }
        };
        let id = if project_id.is_empty() {
            version.project_id.as_str()
        } else {
            project_id
        };
        let project = src.project(id).await.ok()?;
        matches!(project.kind, Some(Kind::Content(_))).then_some((project, version))
    }

    async fn plan_for(
        &self,
        project_id: &str,
        version_id: Option<&str>,
        profile: &Profile,
    ) -> Res<Built> {
        let src = self.source();
        let src = &src;
        let project = src.project(project_id).await?;
        let kind = match project.kind {
            Some(Kind::Content(k)) => k,
            Some(Kind::Modpack) => return Err(use_modpack_install()),
            None => return Err(unsupported_content()),
        };
        let need = loader_need(kind, profile.loader).map_err(|()| {
            AppError::new(
                "no_compatible_version",
                format!(
                    "{} needs a mod loader, the instance is vanilla",
                    project.title
                ),
            )
        })?;
        let mc = profile.mc_version.as_str();
        let version = match version_id {
            Some(v) => src.version(&project.id, v).await?,
            None => {
                let all = src.versions(&project.id, Some(mc), need).await?;
                pick_best(&all, mc, need)
                    .cloned()
                    .ok_or_else(|| no_compatible(&project.title, profile))?
            }
        };

        let new_pick = |project: ProjectInfo, version: VersionInfo, kind: ContentType, role| Pick {
            item: InstallPlanItem {
                project_id: project.id.clone(),
                version_id: version.id.clone(),
                title: project.title.clone(),
                kind,
                icon_url: project
                    .icon
                    .as_deref()
                    .and_then(crate::content::public_icon),
                role,
                already_installed: false,
                installed_version: None,
                size_bytes: version.size,
            },
            project,
            version,
        };

        let mut picks: Vec<Pick> = vec![new_pick(
            project.clone(),
            version,
            kind,
            PlanRole::Requested,
        )];
        let mut index: HashMap<String, usize> = HashMap::from([(project.id.clone(), 0)]);
        let mut incompatible: Vec<(String, String)> = Vec::new();
        let mut queue: VecDeque<usize> = VecDeque::from([0]);
        while let Some(i) = queue.pop_front() {
            // Optional dependencies are leaves: their own needs are not crawled.
            if picks[i].item.role == PlanRole::Optional {
                continue;
            }
            let (deps, title) = (
                picks[i].version.deps.clone(),
                picks[i].project.title.clone(),
            );
            for dep in deps {
                let role = match dep.relation {
                    Relation::Incompatible => {
                        if !dep.project_id.is_empty() {
                            incompatible.push((title.clone(), dep.project_id.clone()));
                        }
                        continue;
                    }
                    Relation::Required => PlanRole::Required,
                    Relation::Optional => PlanRole::Optional,
                };
                if let Some(&j) = index.get(&dep.project_id) {
                    // Seen already (cycle or shared): an optional that turns
                    // out to be required is promoted and crawled.
                    if role == PlanRole::Required && picks[j].item.role == PlanRole::Optional {
                        picks[j].item.role = PlanRole::Required;
                        queue.push_back(j);
                    }
                    continue;
                }
                if picks.len() >= MAX_PLAN_ITEMS {
                    continue;
                }
                let Some((p, v)) = self
                    .resolve_dep(src, &dep.project_id, dep.version_id.as_deref(), profile)
                    .await
                else {
                    tracing::warn!("skipping unresolvable dependency {}", dep.project_id);
                    continue;
                };
                if index.contains_key(&p.id) {
                    continue;
                }
                let dep_kind = match p.kind {
                    Some(Kind::Content(k)) => k,
                    _ => continue,
                };
                index.insert(p.id.clone(), picks.len());
                if role == PlanRole::Required {
                    queue.push_back(picks.len());
                }
                picks.push(new_pick(p, v, dep_kind, role));
            }
        }

        // Installed state.
        for pick in &mut picks {
            if let Some(rec) = installed_record(profile, &pick.project.id) {
                pick.item.installed_version = rec.version_number.clone();
                pick.item.already_installed =
                    rec.version_id.as_deref() == Some(pick.version.id.as_str());
            }
        }

        // Conflicts: an incompatible project that is installed or in the plan.
        let mut conflicts: Vec<PlanConflict> = Vec::new();
        let mut extra_items: Vec<InstallPlanItem> = Vec::new();
        for (with, pid) in incompatible {
            let (title, planned) = match index.get(&pid) {
                Some(&j) => (Some(picks[j].project.title.clone()), true),
                None => (None, false),
            };
            let installed = installed_record(profile, &pid);
            if !planned && installed.is_none() {
                continue;
            }
            let title = title
                .or_else(|| installed.and_then(|r| r.title.clone()))
                .unwrap_or_else(|| pid.clone());
            if conflicts
                .iter()
                .any(|c| c.project_id == pid && c.with == with)
            {
                continue;
            }
            if let (false, Some(rec)) = (planned, installed) {
                if !extra_items.iter().any(|i| i.project_id == pid) {
                    extra_items.push(InstallPlanItem {
                        project_id: pid.clone(),
                        version_id: rec.version_id.clone().unwrap_or_default(),
                        title: title.clone(),
                        kind: rec.kind.unwrap_or(ModType::Mod),
                        icon_url: rec.icon.as_deref().and_then(crate::content::public_icon),
                        role: PlanRole::Incompatible,
                        already_installed: true,
                        installed_version: rec.version_number.clone(),
                        size_bytes: None,
                    });
                }
            }
            conflicts.push(PlanConflict {
                project_id: pid,
                title,
                with,
            });
        }

        picks.sort_by_key(|p| role_rank(p.item.role));
        let total_bytes = picks
            .iter()
            .filter(|p| {
                matches!(p.item.role, PlanRole::Requested | PlanRole::Required)
                    && !p.item.already_installed
            })
            .filter_map(|p| p.item.size_bytes)
            .sum();
        let mut items: Vec<InstallPlanItem> = picks.iter().map(|p| p.item.clone()).collect();
        items.extend(extra_items);
        Ok(Built {
            plan: InstallPlan {
                profile_id: profile.id.clone(),
                items,
                total_bytes,
                conflicts,
            },
            picks,
        })
    }

    pub async fn install_plan(
        &self,
        project_id: String,
        version_id: Option<String>,
        profile_id: String,
    ) -> Res<InstallPlan> {
        let profile = self.profile(&profile_id)?;
        Ok(self
            .plan_for(&project_id, version_id.as_deref(), &profile)
            .await?
            .plan)
    }

    // ---- targets ----

    pub async fn install_targets(
        &self,
        project_id: String,
        version_id: Option<String>,
    ) -> Res<Vec<InstallTarget>> {
        let src = self.source();
        let project = src.project(&project_id).await?;
        let profiles = self.inner.with_store(|s| s.profiles.clone());
        let target = |p: &Profile,
                      state: TargetState,
                      installed_version: Option<String>,
                      reason: Option<TargetReason>,
                      detail: Option<String>,
                      required_deps: u32| InstallTarget {
            profile_id: p.id.clone(),
            state,
            installed_version,
            reason,
            detail,
            required_deps,
        };
        let incompatible = |p: &Profile, reason: TargetReason, detail: String| {
            target(
                p,
                TargetState::Incompatible,
                None,
                Some(reason),
                Some(detail),
                0,
            )
        };

        let kind = match project.kind {
            Some(Kind::Content(k)) => k,
            Some(Kind::Modpack) => {
                return Ok(profiles
                    .iter()
                    .map(|p| {
                        incompatible(
                            p,
                            TargetReason::ContentType,
                            "modpacks are installed as a new instance".into(),
                        )
                    })
                    .collect())
            }
            None => return Err(unsupported_content()),
        };
        let all = match &version_id {
            Some(v) => vec![src.version(&project.id, v).await?],
            None => src.versions(&project.id, None, None).await?,
        };

        let mut out = Vec::with_capacity(profiles.len());
        for p in &profiles {
            if let Some(rec) = installed_record(p, &project.id) {
                out.push(target(
                    p,
                    TargetState::Installed,
                    rec.version_number
                        .clone()
                        .or_else(|| rec.version_id.clone()),
                    None,
                    None,
                    0,
                ));
                continue;
            }
            let need = match loader_need(kind, p.loader) {
                Ok(n) => n,
                Err(()) => {
                    out.push(incompatible(
                        p,
                        TargetReason::Loader,
                        "needs a mod loader, the instance is vanilla".into(),
                    ));
                    continue;
                }
            };
            let mc = p.mc_version.as_str();
            let Some(best) = pick_best(&all, mc, need) else {
                let mc_any = all.iter().any(|v| fits(v, mc, None));
                let loader_any = need.is_some_and(|l| {
                    all.iter()
                        .any(|v| crate::content::loader_accepts(l, &v.loaders))
                });
                out.push(match need {
                    Some(_) if mc_any && !loader_any => {
                        let mut names: Vec<&str> = all
                            .iter()
                            .filter(|v| fits(v, mc, None))
                            .flat_map(|v| v.loaders.iter().map(String::as_str))
                            .collect();
                        names.sort_unstable();
                        names.dedup();
                        incompatible(
                            p,
                            TargetReason::Loader,
                            format!(
                                "needs {}, instance uses {}",
                                names.join(" or "),
                                loader_label(p)
                            ),
                        )
                    }
                    Some(_) if loader_any && !mc_any => incompatible(
                        p,
                        TargetReason::McVersion,
                        format!("no build for Minecraft {mc}"),
                    ),
                    None if !all.is_empty() => incompatible(
                        p,
                        TargetReason::McVersion,
                        format!("no build for Minecraft {mc}"),
                    ),
                    _ => incompatible(
                        p,
                        TargetReason::NoCompatibleVersion,
                        format!("no compatible build for {} {mc}", loader_label(p)),
                    ),
                });
                continue;
            };
            let required = match self.plan_for(&project.id, Some(&best.id), p).await {
                Ok(b) => b
                    .plan
                    .items
                    .iter()
                    .filter(|i| i.role == PlanRole::Required && !i.already_installed)
                    .count() as u32,
                Err(_) => 0,
            };
            out.push(target(
                p,
                TargetState::Available,
                None,
                None,
                None,
                required,
            ));
        }
        Ok(out)
    }

    // ---- apply ----

    pub async fn install_apply(
        &self,
        project_id: String,
        version_id: Option<String>,
        profile_id: String,
        include: Vec<String>,
    ) -> Res<Vec<ContentItem>> {
        let _busy = self.busy_guard();
        let profile = self.profile(&profile_id)?;
        let built = self
            .plan_for(&project_id, version_id.as_deref(), &profile)
            .await?;
        let Some(requested) = built.picks.first() else {
            return Err(unsupported_content());
        };
        let requested_id = requested.project.id.clone();
        let reporter = InstallReporter::new(&self.inner, &profile_id, &requested_id);

        let wanted: HashSet<&str> = include.iter().map(String::as_str).collect();
        let selected: Vec<&Pick> = built
            .picks
            .iter()
            .filter(|p| match p.item.role {
                PlanRole::Requested | PlanRole::Required => true,
                PlanRole::Optional => wanted.contains(p.project.id.as_str()),
                PlanRole::Incompatible => false,
            })
            .collect();
        let to_install: Vec<&Pick> = selected
            .iter()
            .copied()
            .filter(|p| !p.item.already_installed)
            .collect();
        // Replacing a build that is installed already: keep a way back.
        if let Some(replaced) = to_install
            .iter()
            .find(|p| installed_record(&profile, &p.project.id).is_some())
        {
            self.auto_snapshot(
                &profile_id,
                crate::backups::BackupReason::BeforeUpdate,
                format!("Before updating {}", replaced.item.title),
            )
            .await;
        }
        let total = to_install.len() as u64;
        let by_project: HashMap<&str, &str> = to_install
            .iter()
            .map(|p| (p.project.id.as_str(), p.version.id.as_str()))
            .collect();
        let by_version: HashMap<&str, &str> = to_install
            .iter()
            .map(|p| (p.version.id.as_str(), p.project.id.as_str()))
            .collect();

        reporter.emit(0, total, &built.picks[0].item.title);
        let src = self.source();
        let mut installed: Vec<ModRecord> = Vec::new();
        let mut failure: Option<AppError> = None;
        for (n, pick) in to_install.iter().enumerate() {
            reporter.emit(n as u64, total, &pick.item.title);
            match self
                .install_one(
                    &src,
                    &profile,
                    pick,
                    &requested_id,
                    &by_project,
                    &by_version,
                )
                .await
            {
                Ok(rec) => installed.push(rec),
                Err(e) => {
                    failure = Some(e);
                    break;
                }
            }
        }

        // Keep the records of everything that did land on disk, even when a
        // later download failed, so no jar is left unknown to the profile.
        let mut merged = self.contain_records(&profile_id, &self.profile(&profile_id)?.mods);
        if !installed.is_empty() {
            let touched: Vec<String> = installed
                .iter()
                .filter_map(|r| r.project_id.clone())
                .collect();
            merged = modrinth::merge_installed(&merged, installed).await;
            self.save_mods(&profile_id, &merged)?;
            lock(&self.inner.updates)
                .entry(profile_id.clone())
                .and_modify(|m| m.retain(|k, _| !touched.contains(k)));
        }
        if let Some(e) = failure {
            return Err(e);
        }
        reporter.emit(total, total, "done");
        Ok(self.items(&profile_id, &merged))
    }

    async fn install_one(
        &self,
        src: &ModrinthSource,
        profile: &Profile,
        pick: &Pick,
        requested_id: &str,
        by_project: &HashMap<&str, &str>,
        by_version: &HashMap<&str, &str>,
    ) -> Res<ModRecord> {
        let (project, version, item) = (&pick.project, &pick.version, &pick.item);
        let url = src.download_url(project, version).await?.ok_or_else(|| {
            AppError::new(
                "modrinth.no_download",
                format!("{} has no downloadable file", item.title),
            )
        })?;
        let dir = modrinth::target_dir_for(&self.inner.paths, item.kind, &profile.id)?;
        let name = safe_name(&version.filename)?;
        let dest = safe_dest(&dir, name)?;
        download_file(
            &url,
            &dest,
            &DownloadOptions {
                expected: version.expected.clone(),
                ..DownloadOptions::default()
            },
        )
        .await?;

        // Dependencies that are (or just became) installed. Modrinth pins the
        // exact build the parent asked for.
        let mut refs: Vec<mango_core::ModDependency> = Vec::new();
        for d in version
            .deps
            .iter()
            .filter(|d| d.relation == Relation::Required)
        {
            let pid = if d.project_id.is_empty() {
                d.version_id
                    .as_deref()
                    .and_then(|v| by_version.get(v))
                    .map(|p| p.to_string())
                    .unwrap_or_default()
            } else {
                d.project_id.clone()
            };
            if pid.is_empty() || refs.iter().any(|r| r.project_id == pid) {
                continue;
            }
            let present =
                by_project.contains_key(pid.as_str()) || installed_record(profile, &pid).is_some();
            if !present {
                continue;
            }
            let version_id = d
                .version_id
                .clone()
                .or_else(|| by_project.get(pid.as_str()).map(|v| v.to_string()));
            refs.push(mango_core::ModDependency {
                project_id: pid,
                version_id,
            });
        }
        Ok(ModRecord {
            project_id: Some(project.id.clone()),
            slug: Some(project.slug.clone()),
            title: Some(project.title.clone()),
            kind: Some(item.kind),
            icon: project.icon.clone(),
            version_id: Some(version.id.clone()),
            version_number: Some(version.number.clone()),
            filename: Some(name.to_string()),
            file: Some(dest.to_string_lossy().into_owned()),
            game_versions: Some(version.game_versions.clone()),
            loaders: Some(version.loaders.clone()),
            installed_at: Some(now_ms()),
            dependency: Some(project.id != requested_id),
            required_dependencies: Some(refs),
            ..ModRecord::default()
        })
    }

    // ---- modpacks ----

    pub async fn modpack_plan(
        &self,
        project_id: String,
        version_id: Option<String>,
    ) -> Res<ModpackPlan> {
        self.modrinth_pack_plan(&project_id, version_id.as_deref())
            .await
    }

    pub async fn modpack_install(
        &self,
        project_id: String,
        version_id: Option<String>,
        name: Option<String>,
    ) -> Res<ProfileView> {
        let _busy = self.busy_guard();
        self.modrinth_install_modpack(project_id, version_id, name)
            .await
    }

    async fn modrinth_pack_plan(
        &self,
        project_id: &str,
        version_id: Option<&str>,
    ) -> Res<ModpackPlan> {
        let src = self.source();
        let project = src.project(project_id).await?;
        if project.kind != Some(Kind::Modpack) {
            return Err(not_a_modpack());
        }
        let version = match version_id {
            Some(v) => src.version(&project.id, v).await?,
            None => {
                let all = src.versions(&project.id, None, None).await?;
                pick_latest(&all).cloned().ok_or_else(|| {
                    AppError::new("no_compatible_version", "The modpack has no versions")
                })?
            }
        };
        let url = src
            .download_url(&project, &version)
            .await?
            .ok_or_else(|| AppError::new("modrinth.no_download", "The modpack has no file"))?;
        // Same path and expectations as the installer: it finds the file
        // valid and does not download it a second time.
        let cache = self.inner.paths.cache();
        let dest = safe_dest(&cache, &version.filename)?;
        download_file(
            &url,
            &dest,
            &DownloadOptions {
                expected: version.expected.clone(),
                ..DownloadOptions::default()
            },
        )
        .await?;
        let scratch = cache.join(".inspect-nonexistent");
        let d2 = dest.clone();
        let summary = tokio::task::spawn_blocking(move || modrinth::inspect_pack(&d2, &scratch))
            .await?
            .inspect_err(|_| {
                let _ = std::fs::remove_file(&dest);
            })?;
        let info = summary.info;
        let mc_version = info
            .mc_version
            .filter(|m| !m.is_empty())
            .or_else(|| version.game_versions.first().cloned())
            .ok_or_else(|| {
                AppError::new(
                    "modrinth.invalid_modpack",
                    "The pack names no Minecraft version",
                )
            })?;
        Ok(ModpackPlan {
            project_id: project.id,
            version_id: version.id,
            name: project.title,
            mc_version,
            loader: info.loader,
            loader_version: (!info.loader_version.is_empty()).then_some(info.loader_version),
            file_count: info.file_count as u64,
            size_bytes: Some(summary.download_bytes),
            icon_url: project
                .icon
                .as_deref()
                .and_then(crate::content::public_icon),
            ram: None,
            applied_ram_mb: None,
        })
    }
}

/// What a pack's memory hint gives a new instance on this machine (the plan
/// dialogs and the installer both go through here, so they always agree).
pub(crate) fn applied_ram_mb(hint: Option<&mango_core::RamHint>) -> Option<u32> {
    applied_ram_mb_for(hint, mango_core::model::total_ram_mb())
}

pub(crate) fn applied_ram_mb_for(hint: Option<&mango_core::RamHint>, total_mb: u64) -> Option<u32> {
    hint.map(|h| h.applied_mb(&mango_core::ram_limits(total_mb)))
}

/// Adds the creation-time RAM fields to a profile patch.
pub(crate) fn ram_patch(patch: &mut Value, hint: &mango_core::RamHint, mb: u32) {
    patch["ram"] = json!(mb);
    patch["ramSource"] = json!("modpack");
    patch["ramHint"] = json!(hint);
}
