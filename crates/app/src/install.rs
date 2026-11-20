//! Part 4 of `docs/ipc.md`: providers, install planning, modpacks.
//!
//! Browsing and planning are provider-agnostic (see `providers.rs`);
//! persistence goes through the store exactly like the legacy Modrinth path.

use crate::blocked::BlockedEntry;
use crate::content::{is_curseforge, InstallReporter, PROJECT_TYPES};
use crate::error::{AppError, Res};
use crate::events::{Event, LaunchLogEvent, LogLevelName};
use crate::models::{
    ContentItem, ContentType, InstallPlan, InstallPlanItem, InstallTarget, ModpackPlan,
    PlanConflict, PlanRole, ProjectDetail, ProjectVersion, Provider, ProviderStatus, SearchParams,
    SearchResult, StatusReason, TargetReason, TargetState, UpdateInfo,
};
use crate::providers::{
    cf_id, check_sort, fits, loader_need, pick_best, pick_latest, CurseSource, InstalledIndex,
    Kind, ModrinthSource, ProjectInfo, Relation, Source, VersionInfo,
};
use crate::service::{lock, now_ms, AppService};
use crate::types::ProfileView;
use mango_core::{ModRecord, ModType, Profile};
use mango_curseforge as cf;
use mango_modrinth as modrinth;
use mango_net::{download_file, pool, DownloadOptions};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Arc;

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

fn distribution_blocked(title: &str, url: Option<&str>) -> AppError {
    AppError::new(
        "distribution_blocked",
        match url {
            Some(u) => format!("{title} may not be downloaded by other launchers; get it at {u}"),
            None => format!("{title} may not be downloaded by other launchers"),
        },
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

fn installed_record<'a>(
    profile: &'a Profile,
    provider: Provider,
    project_id: &str,
) -> Option<&'a ModRecord> {
    profile.mods.iter().find(|r| {
        Provider::of_record(r) == Some(provider) && r.project_id.as_deref() == Some(project_id)
    })
}

fn loader_label(profile: &Profile) -> &'static str {
    modrinth::loader_name(profile.loader)
}

impl AppService {
    // ---- key and sources ----

    /// The CurseForge key in force: the config field wins over the build-time
    /// or environment key. Malformed keys count as missing.
    fn curseforge_key(&self) -> Option<String> {
        let from_config = self
            .inner
            .with_store(|s| s.config.curseforge_api_key.clone())
            .map(|k| k.expose().to_string());
        from_config
            .or_else(|| {
                self.inner
                    .opts
                    .curseforge_key
                    .as_ref()
                    .map(|k| k.expose().to_string())
            })
            .map(|k| k.trim().to_string())
            .filter(|k| cf::ApiKey::is_valid_format(k))
    }

    pub(crate) fn curseforge_client(&self) -> Res<Arc<cf::Client>> {
        let key = self.curseforge_key().ok_or_else(|| {
            AppError::new(
                "curseforge.no_api_key",
                "CurseForge is not available: no API key",
            )
        })?;
        let mut slot = lock(&self.inner.curseforge);
        if let Some((k, c)) = slot.as_ref() {
            if *k == key {
                return Ok(c.clone());
            }
        }
        let opts = &self.inner.opts;
        let client = Arc::new(cf::Client::with_base(
            cf::ApiKey::new(&key)?,
            &opts.curseforge_api,
            opts.curseforge.clone(),
        )?);
        *slot = Some((key, client.clone()));
        Ok(client)
    }

    pub(crate) fn source(&self, provider: Provider) -> Res<Box<dyn Source>> {
        Ok(match provider {
            Provider::Modrinth => Box::new(ModrinthSource {
                client: self.inner.modrinth.clone(),
                fetch: self.inner.opts.fetch.clone(),
            }),
            Provider::Curseforge => Box::new(CurseSource {
                client: self.curseforge_client()?,
            }),
        })
    }

    /// Project id -> profiles that have it from `provider`.
    fn installed_index(&self, provider: Provider) -> InstalledIndex {
        self.inner.with_store(|s| {
            let mut index: InstalledIndex = HashMap::new();
            for p in &s.profiles {
                for m in &p.mods {
                    if Provider::of_record(m) != Some(provider) {
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

    pub fn providers_status(&self) -> Res<Vec<ProviderStatus>> {
        let cf_ok = self.curseforge_key().is_some();
        Ok(vec![
            ProviderStatus {
                provider: Provider::Modrinth,
                available: true,
                reason: None,
            },
            ProviderStatus {
                provider: Provider::Curseforge,
                available: cf_ok,
                reason: (!cf_ok).then_some(StatusReason::NoApiKey),
            },
        ])
    }

    pub async fn search_projects(&self, params: SearchParams) -> Res<SearchResult> {
        if !PROJECT_TYPES.contains(&params.project_type.as_str()) {
            return Err(AppError::validation("Unknown project type"));
        }
        check_sort(&params.sort)?;
        let src = self.source(params.provider)?;
        let installed = self.installed_index(params.provider);
        src.search(&params, &installed).await
    }

    pub async fn project_detail(
        &self,
        provider: Provider,
        project_id: String,
    ) -> Res<ProjectDetail> {
        let src = self.source(provider)?;
        let installed = self.installed_index(provider);
        src.detail(&project_id, &installed).await
    }

    pub async fn project_versions(
        &self,
        provider: Provider,
        project_id: String,
        profile_id: Option<String>,
    ) -> Res<Vec<ProjectVersion>> {
        let profile = match profile_id {
            Some(id) => Some(self.profile(&id)?),
            None => None,
        };
        let src = self.source(provider)?;
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
                    provider,
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
        src: &dyn Source,
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
        provider: Provider,
        project_id: &str,
        version_id: Option<&str>,
        profile: &Profile,
    ) -> Res<Built> {
        let src = self.source(provider)?;
        let src: &dyn Source = &*src;
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
                provider,
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
                distribution_blocked: false,
                size_bytes: version.size,
                download_url: None,
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

        // Installed state, distribution and sizes.
        for pick in &mut picks {
            if let Some(rec) = installed_record(profile, provider, &pick.project.id) {
                pick.item.installed_version = rec.version_number.clone();
                pick.item.already_installed =
                    rec.version_id.as_deref() == Some(pick.version.id.as_str());
            }
            if pick.item.already_installed {
                continue;
            }
            let url = if pick.project.blocked {
                None
            } else {
                src.download_url(&pick.project, &pick.version).await?
            };
            if url.is_none() {
                pick.item.distribution_blocked = true;
                pick.item.download_url = pick.project.website.clone();
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
            let installed = installed_record(profile, provider, &pid);
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
                        provider,
                        project_id: pid.clone(),
                        version_id: rec.version_id.clone().unwrap_or_default(),
                        title: title.clone(),
                        kind: rec.kind.unwrap_or(ModType::Mod),
                        icon_url: rec.icon.as_deref().and_then(crate::content::public_icon),
                        role: PlanRole::Incompatible,
                        already_installed: true,
                        installed_version: rec.version_number.clone(),
                        distribution_blocked: false,
                        size_bytes: None,
                        download_url: None,
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
                    && !p.item.distribution_blocked
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
        provider: Provider,
        project_id: String,
        version_id: Option<String>,
        profile_id: String,
    ) -> Res<InstallPlan> {
        let profile = self.profile(&profile_id)?;
        Ok(self
            .plan_for(provider, &project_id, version_id.as_deref(), &profile)
            .await?
            .plan)
    }

    // ---- targets ----

    pub async fn install_targets(
        &self,
        provider: Provider,
        project_id: String,
        version_id: Option<String>,
    ) -> Res<Vec<InstallTarget>> {
        let src = self.source(provider)?;
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
            if let Some(rec) = installed_record(p, provider, &project.id) {
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
            if project.blocked {
                out.push(incompatible(
                    p,
                    TargetReason::DistributionBlocked,
                    project
                        .website
                        .clone()
                        .unwrap_or_else(|| "the author blocks third-party downloads".into()),
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
            if src.download_url(&project, best).await?.is_none() {
                out.push(incompatible(
                    p,
                    TargetReason::DistributionBlocked,
                    project
                        .website
                        .clone()
                        .unwrap_or_else(|| "the author blocks third-party downloads".into()),
                ));
                continue;
            }
            let required = match self
                .plan_for(provider, &project.id, Some(&best.id), p)
                .await
            {
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
        provider: Provider,
        project_id: String,
        version_id: Option<String>,
        profile_id: String,
        include: Vec<String>,
    ) -> Res<Vec<ContentItem>> {
        let profile = self.profile(&profile_id)?;
        let built = self
            .plan_for(provider, &project_id, version_id.as_deref(), &profile)
            .await?;
        let Some(requested) = built.picks.first() else {
            return Err(unsupported_content());
        };
        if requested.item.distribution_blocked {
            return Err(distribution_blocked(
                &requested.item.title,
                requested.item.download_url.as_deref(),
            ));
        }
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
        let mut to_install: Vec<&Pick> = Vec::new();
        for p in &selected {
            if p.item.distribution_blocked {
                self.warn(
                    &profile_id,
                    format!(
                        "Skipped {}: the author blocks third-party downloads{}",
                        p.item.title,
                        p.item
                            .download_url
                            .as_deref()
                            .map(|u| format!(" (get it at {u})"))
                            .unwrap_or_default()
                    ),
                );
            } else if !p.item.already_installed {
                to_install.push(p);
            }
        }
        // Replacing a build that is installed already: keep a way back.
        if let Some(replaced) = to_install
            .iter()
            .find(|p| installed_record(&profile, p.item.provider, &p.project.id).is_some())
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
        let src = self.source(provider)?;
        let mut installed: Vec<ModRecord> = Vec::new();
        let mut failure: Option<AppError> = None;
        for (n, pick) in to_install.iter().enumerate() {
            reporter.emit(n as u64, total, &pick.item.title);
            match self
                .install_one(
                    &*src,
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
        let mut merged = self.profile(&profile_id)?.mods;
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
        src: &dyn Source,
        profile: &Profile,
        pick: &Pick,
        requested_id: &str,
        by_project: &HashMap<&str, &str>,
        by_version: &HashMap<&str, &str>,
    ) -> Res<ModRecord> {
        let (project, version, item) = (&pick.project, &pick.version, &pick.item);
        let url = src
            .download_url(project, version)
            .await?
            .ok_or_else(|| distribution_blocked(&item.title, item.download_url.as_deref()))?;
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
        // exact build the parent asked for; CurseForge never pins.
        let provider = item.provider;
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
            let present = by_project.contains_key(pid.as_str())
                || installed_record(profile, provider, &pid).is_some();
            if !present {
                continue;
            }
            let version_id = match provider {
                Provider::Modrinth => d
                    .version_id
                    .clone()
                    .or_else(|| by_project.get(pid.as_str()).map(|v| v.to_string())),
                Provider::Curseforge => None,
            };
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
            provider: Some(provider.as_str().to_string()),
            ..ModRecord::default()
        })
    }

    // ---- CurseForge update check ----

    /// Newer builds of the profile's CurseForge items. Never fails: a missing
    /// key or an unreachable API just yields no updates.
    pub(crate) async fn check_curseforge_updates(&self, profile: &Profile) -> Vec<UpdateInfo> {
        let items: Vec<ModRecord> = profile
            .mods
            .iter()
            .filter(|r| {
                is_curseforge(r)
                    && r.local != Some(true)
                    && r.project_id.is_some()
                    && matches!(
                        r.kind.unwrap_or(ModType::Mod),
                        ModType::Mod | ModType::Resourcepack | ModType::Shader
                    )
            })
            .cloned()
            .collect();
        if items.is_empty() {
            return Vec::new();
        }
        let Ok(client) = self.curseforge_client() else {
            return Vec::new();
        };
        let src = Arc::new(CurseSource { client });
        let (mc, loader) = (profile.mc_version.clone(), profile.loader);
        let results = pool(items, 6, move |rec, _| {
            let (src, mc) = (src.clone(), mc.clone());
            async move {
                let kind = rec.kind.unwrap_or(ModType::Mod);
                let need = loader_need(kind, loader).ok()?;
                let id = rec.project_id.clone()?;
                let versions = src.versions(&id, Some(&mc), need).await.ok()?;
                let best = pick_best(&versions, &mc, need)?;
                if Some(&best.id) == rec.version_id.as_ref() {
                    return None;
                }
                Some(UpdateInfo {
                    provider: Some(Provider::Curseforge),
                    project_id: id,
                    title: rec.title.clone().unwrap_or_default(),
                    kind,
                    from: rec.version_number.clone(),
                    to: best.number.clone(),
                    version_id: best.id.clone(),
                    compatibility: true,
                    conflict: false,
                    required_by: Vec::new(),
                })
            }
        })
        .await
        .unwrap_or_default();
        results.into_iter().flatten().collect()
    }

    // ---- modpacks ----

    pub async fn modpack_plan(
        &self,
        provider: Provider,
        project_id: String,
        version_id: Option<String>,
    ) -> Res<ModpackPlan> {
        match provider {
            Provider::Modrinth => {
                self.modrinth_pack_plan(&project_id, version_id.as_deref())
                    .await
            }
            Provider::Curseforge => Ok(self
                .curse_pack_prepare(&project_id, version_id.as_deref(), None)
                .await?
                .plan),
        }
    }

    pub async fn modpack_install(
        &self,
        provider: Provider,
        project_id: String,
        version_id: Option<String>,
        name: Option<String>,
    ) -> Res<ProfileView> {
        match provider {
            Provider::Modrinth => {
                self.modrinth_install_modpack(project_id, version_id, name)
                    .await
            }
            Provider::Curseforge => self.curse_pack_install(project_id, version_id, name).await,
        }
    }

    async fn modrinth_pack_plan(
        &self,
        project_id: &str,
        version_id: Option<&str>,
    ) -> Res<ModpackPlan> {
        let src = self.source(Provider::Modrinth)?;
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
            provider: Provider::Modrinth,
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
            blocked_files: Vec::new(),
        })
    }

    /// Download the pack zip, read its manifest and look every file up.
    async fn curse_pack_prepare(
        &self,
        project_id: &str,
        version_id: Option<&str>,
        progress: Option<mango_net::ProgressFn>,
    ) -> Res<CursePack> {
        let client = self.curseforge_client()?;
        let n = cf_id(project_id)?;
        let project = client.project(n).await?;
        if project.class() != Some(cf::ClassId::Modpacks) {
            return Err(not_a_modpack());
        }
        if project.distribution_blocked() {
            return Err(distribution_blocked(
                &project.name,
                project.website_url().as_deref(),
            ));
        }
        let file = match version_id {
            Some(v) => client.file(n, cf_id(v)?).await?,
            None => client
                .files(n, None)
                .await?
                .into_iter()
                .filter(|f| f.is_available() && f.is_server_pack != Some(true))
                .max_by(|a, b| {
                    b.release()
                        .cmp(&a.release())
                        .then_with(|| a.file_date.cmp(&b.file_date))
                })
                .ok_or_else(|| {
                    AppError::new("no_compatible_version", "The modpack has no files")
                })?,
        };
        let cache = self.inner.paths.cache();
        let zip = safe_dest(&cache, &file.file_name)?;
        cf::download_cf_file(&client, Some(&project), &file, &zip, progress).await?;
        let prepared = async {
            let z = zip.clone();
            let manifest = tokio::task::spawn_blocking(move || cf::read_manifest(&z))
                .await?
                .map_err(AppError::from)?;
            let resolved = cf::resolve_pack(&client, &manifest).await?;
            Ok::<_, AppError>((manifest, resolved))
        }
        .await;
        let (manifest, resolved) = match prepared {
            Ok(v) => v,
            Err(e) => {
                let _ = std::fs::remove_file(&zip);
                return Err(e);
            }
        };
        let blocked = self.blocked_entries(&resolved).await;
        let plan = ModpackPlan {
            provider: Provider::Curseforge,
            project_id: project.id.to_string(),
            version_id: file.id.to_string(),
            name: project.name.clone(),
            mc_version: manifest.mc_version.clone(),
            loader: manifest.loader,
            loader_version: (!manifest.loader_version.is_empty())
                .then(|| manifest.loader_version.clone()),
            file_count: (resolved.entries.len() + resolved.missing.len()) as u64,
            size_bytes: Some(resolved.download_bytes()),
            icon_url: project
                .logo
                .as_ref()
                .and_then(|l| l.thumbnail_url.as_deref().or(l.url.as_deref()))
                .and_then(crate::content::public_icon),
            blocked_files: blocked.iter().map(|b| b.file.clone()).collect(),
        };
        Ok(CursePack {
            plan,
            zip,
            resolved,
            blocked,
        })
    }

    async fn curse_pack_install(
        &self,
        project_id: String,
        version_id: Option<String>,
        name: Option<String>,
    ) -> Res<ProfileView> {
        let client = self.curseforge_client()?;
        let n = cf_id(&project_id)?;
        let project = client.project(n).await?;
        if project.class() != Some(cf::ClassId::Modpacks) {
            return Err(not_a_modpack());
        }
        // Provisional values; the manifest says what the pack really needs.
        let (mc, loader) = match &version_id {
            Some(v) => match client.file(n, cf_id(v)?).await {
                Ok(f) => (
                    f.minecraft_versions().next().map(str::to_string),
                    f.loaders().first().map(|l| l.tag().to_ascii_lowercase()),
                ),
                Err(_) => (None, None),
            },
            None => (None, None),
        };
        let name: String = name
            .map(|n| n.trim().to_string())
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| project.name.clone())
            .chars()
            .take(128)
            .collect();
        let created = self.inner.with_store(|s| {
            s.add_profile(&json!({
                "name": name,
                "mcVersion": mc.unwrap_or_else(|| "1.21.1".into()),
                "loader": loader.unwrap_or_else(|| "vanilla".into()),
            }))
            .or_else(|_| {
                s.add_profile(&json!({"name": name, "mcVersion": "1.21.1", "loader": "vanilla"}))
            })
        })?;
        match self
            .finish_curse_pack(&created, &project_id, version_id.as_deref())
            .await
        {
            Ok(view) => {
                let logo = project
                    .logo
                    .as_ref()
                    .and_then(|l| l.url.as_deref().or(l.thumbnail_url.as_deref()));
                self.apply_pack_logo(&created.id, logo).await;
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

    async fn finish_curse_pack(
        &self,
        profile: &Profile,
        project_id: &str,
        version_id: Option<&str>,
    ) -> Res<ProfileView> {
        let reporter = InstallReporter::new(&self.inner, &profile.id, project_id);
        reporter.emit(0, 1, "modpack");
        let zip_progress: mango_net::ProgressFn = {
            let r = reporter.clone();
            Arc::new(move |done, total| r.emit_throttled(done, total.max(done), "modpack"))
        };
        let mut pack = self
            .curse_pack_prepare(project_id, version_id, Some(zip_progress))
            .await?;
        let result = self.install_curse_pack(profile, &reporter, &mut pack).await;
        let _ = std::fs::remove_file(&pack.zip);
        result
    }

    async fn install_curse_pack(
        &self,
        profile: &Profile,
        reporter: &InstallReporter,
        pack: &mut CursePack,
    ) -> Res<ProfileView> {
        let m = &pack.resolved.manifest;
        let mut patch = json!({"loader": m.loader, "loaderVersion": m.loader_version});
        patch["mcVersion"] = Value::String(m.mc_version.clone());
        let updated = self
            .inner
            .with_store(|s| s.update_profile(&profile.id, &patch).map(|p| p.cloned()))?
            .ok_or_else(AppError::profile_not_found)?;

        for f in &pack.resolved.missing {
            self.warn(
                &profile.id,
                format!(
                    "Skipped file {} of project {}: not found",
                    f.file_id, f.project_id
                ),
            );
        }
        let game_dir = self.inner.paths.instance_dir(&profile.id)?;
        let progress: Arc<cf::ProgressFn> = {
            let r = reporter.clone();
            Arc::new(move |p: &cf::PackProgress| {
                r.emit_throttled(p.done as u64, p.total as u64, &p.label)
            })
        };
        cf::install_resolved(&pack.resolved, &pack.zip, &game_dir, Some(progress)).await?;

        // Blocked files Modrinth also hosts come from there; the rest wait
        // for the user (see `blocked.rs`).
        let modrinth_records = {
            let r = reporter.clone();
            let report = move |done: u64, total: u64, label: &str| {
                r.emit_throttled(done, total.max(done), label)
            };
            self.install_modrinth_matches(&profile.id, &mut pack.blocked, &report)
                .await
        };
        for b in pack
            .blocked
            .iter()
            .filter(|b| b.file.state == crate::models::BlockedState::Pending)
        {
            self.warn(
                &profile.id,
                format!("Skipped {}: get it at {}", b.file.file_name, b.file.url),
            );
        }
        let files: Vec<_> = pack.blocked.iter().map(|b| b.file.clone()).collect();
        self.store_blocked(&profile.id, &files)?;

        // The pack's jars become ordinary (local) content records; the ones
        // fetched from Modrinth keep their own.
        let mut snapshot = updated;
        snapshot.mods.extend(modrinth_records);
        let paths = self.inner.paths.clone();
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

struct CursePack {
    plan: ModpackPlan,
    zip: PathBuf,
    resolved: cf::ResolvedPack,
    blocked: Vec<BlockedEntry>,
}
