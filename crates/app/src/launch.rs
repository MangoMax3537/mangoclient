//! `game_launch`: the CLI's pipeline on a background task, with events,
//! playtime bookkeeping and adoption of sessions that outlived the launcher.

use crate::error::{AppError, Res};
use crate::events::{Event, LaunchLogEvent, LaunchProgressEvent, LogLevelName, ProgressPhase};
use crate::service::{lock, now_ms, AppService, Inner};
use crate::types::{LaunchResult, LaunchState};
use mango_content::{ContentError, ModrinthLookup, PerfOptions, ResolvedFile};
use mango_core::{validate_profile_id, Config, Profile};
use mango_java::{launch_executable, resolve_java, JavaInfo, Platform};
use mango_loaders::{Loader, Loaders};
use mango_mango::mangoconfig::{self, Bundle};
use mango_minecraft::{
    build_launch_command, install_version, resolve_version, HostEnv, InstallOptions,
    InstallProgress, LaunchInput, Phase, Preset,
};
use mango_runtime::{
    is_session_alive, log_file_for, AdoptOptions, EventCb, GameEvent, GameInstance, GameState,
    LogLevel, SpawnOptions,
};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Weak};
use std::time::Instant;

/// Removes the profile from the "launching" set however the task ends.
struct LaunchGuard {
    inner: Arc<Inner>,
    id: String,
}

impl Drop for LaunchGuard {
    fn drop(&mut self) {
        lock(&self.inner.launching).remove(&self.id);
        lock(&self.inner.launch_cancel).remove(&self.id);
    }
}

impl AppService {
    /// Runs the pipeline on a background task; returns once the game process
    /// is spawned. Progress and state go out as events.
    pub async fn game_launch(
        &self,
        profile_id: String,
        quick_join: Option<String>,
    ) -> Res<LaunchResult> {
        validate_profile_id(&profile_id)?;
        let inner = &self.inner;
        let profile = inner
            .with_store(|s| s.get_profile(&profile_id).cloned())
            .ok_or_else(AppError::profile_not_found)?;

        if lock(&inner.games).contains_key(&profile_id)
            || !lock(&inner.launching).insert(profile_id.clone())
        {
            return Err(AppError::already_running());
        }
        let guard = LaunchGuard {
            inner: Arc::clone(inner),
            id: profile_id.clone(),
        };
        if let Some(session) = &profile.session {
            if is_session_alive(session) {
                return Err(AppError::already_running());
            }
            let _ = inner.patch_profile(&profile_id, &json!({"session": null}));
        }

        let cancel = Arc::new(crate::service::LaunchCancel::default());
        lock(&inner.launch_cancel).insert(profile_id.clone(), Arc::clone(&cancel));

        let svc = self.clone();
        let task = tokio::spawn(async move {
            let _guard = guard;
            let id = profile.id.clone();
            // Stop wins at any await point: dropping the pipeline aborts its HTTP transfers (partial files are removed by
            // their temp guards), its download pool and any installer child (kill_on_drop). The game process is spawned
            // and registered without an await in between, so it is never orphaned; once it runs, Stop goes to it.
            let result = tokio::select! {
                biased;
                () = cancel.cancelled() => Err(AppError::launch_cancelled()),
                r = svc.run_pipeline(profile, quick_join) => r,
            };
            if result.as_ref().is_err_and(|e| e.code == "launch_cancelled") {
                svc.inner.emit(Event::LaunchLog(LaunchLogEvent {
                    profile_id: id.clone(),
                    line: "Launch stopped".to_string(),
                    level: LogLevelName::Info,
                }));
                svc.inner.set_state(&id, LaunchState::Stopped);
            } else if let Err(e) = &result {
                svc.inner.emit(Event::LaunchLog(LaunchLogEvent {
                    profile_id: id.clone(),
                    line: format!("Launch failed: {} ({})", e.message, e.code),
                    level: LogLevelName::Error,
                }));
                svc.inner.set_state(&id, LaunchState::Stopped);
            }
            result
        });
        task.await?
    }

    async fn run_pipeline(
        &self,
        profile: Profile,
        quick_join: Option<String>,
    ) -> Res<LaunchResult> {
        let inner = &self.inner;
        let paths = &inner.paths;
        let id = profile.id.clone();
        let state = |s: LaunchState| inner.set_state(&id, s);
        let log = |line: &str| {
            inner.emit(Event::LaunchLog(LaunchLogEvent {
                profile_id: id.clone(),
                line: line.to_string(),
                level: LogLevelName::Info,
            }));
        };

        // Account first: nothing is emitted or downloaded without one.
        let (config, account_uuid) = inner.with_store(|s| {
            (
                s.config.clone(),
                s.selected_account().map(|a| a.uuid.clone()),
            )
        });
        let account_uuid = account_uuid.ok_or_else(|| {
            AppError::new("no_account", "No account selected; add an account first")
        })?;

        state(LaunchState::Preparing);
        // Microsoft accounts get a fresh token (and a persisted rotated
        // refresh token) up front: a session that needs a new login fails
        // here, before anything is downloaded.
        let creds = self.launch_credentials(&account_uuid).await?;
        let mut profile = profile;
        let bundle_dir = inner.opts.bundle_dir.clone();
        // Vanilla switches to Fabric when MangoConfig can be injected.
        if mangoconfig::should_switch_vanilla_to_fabric(
            &profile,
            &config,
            &Bundle::scan(&bundle_dir),
        ) {
            log("Switching vanilla profile to Fabric for MangoConfig");
            let updated = inner.with_store(|s| {
                s.update_profile(&id, &json!({"loader": "fabric", "loaderVersion": ""}))
                    .map(|p| p.cloned())
            })?;
            if let Some(p) = updated {
                profile = p;
            }
        }

        // Neither of these ever blocks a launch.
        let outcome =
            mangoconfig::ensure_with_log(paths, &profile, &config, &bundle_dir, &|l| log(l));
        log(&format!("MangoConfig: {outcome:?}"));
        if config.performance_mods {
            let lookup = ModrinthPerf(mango_modrinth::Client::with_options(
                &inner.opts.modrinth_api,
                inner.opts.fetch.clone(),
            ));
            let perf = mango_content::ensure_performance_mods(
                paths,
                &PerfOptions {
                    profile: &profile,
                    global_enabled: true,
                    profile_enabled: None,
                },
                &lookup,
            )
            .await;
            for l in &perf.logs {
                log(l);
            }
        }

        let game_dir = paths.instance_dir(&id)?;
        if let Err(e) = seed_options(&game_dir, &config) {
            log(&format!("Could not seed options.txt: {e}"));
        }
        let icons = mango_servers::ServerIcons::new(paths.server_icons_file());
        if let Err(e) =
            mango_servers::seed_featured_servers(&game_dir, &inner.opts.partners, &icons).await
        {
            log(&format!("Partnered server list skipped: {e}"));
        }

        // 1. Java against the vanilla metadata (installers need a JVM first).
        state(LaunchState::Java);
        let fetch = inner.opts.fetch.clone();
        let base =
            resolve_version(paths, &profile.mc_version, &inner.opts.manifest_url, &fetch).await?;
        let required = mango_java::required_major_from_json(&base.version);
        let java = self
            .resolve_java_for(&id, &config, profile.java_path.as_deref(), required)
            .await?;
        log(&format!(
            "Java {} ({:?}): {}",
            java.major,
            java.source,
            java.path.display()
        ));

        // 2. Loader
        state(LaunchState::Loader);
        let loader = Loader::parse(profile.loader_str())
            .ok_or_else(|| AppError::new("loaders.unsupported", "Unsupported loader"))?;
        let java_info = JavaInfo {
            path: java.path.clone(),
            major: java.major,
            raw: String::new(),
        };
        let wanted =
            (!profile.loader_version.is_empty()).then_some(profile.loader_version.as_str());
        let installed = Loaders::new(inner.opts.loader_endpoints.clone())
            .ensure(
                paths,
                loader,
                &profile.mc_version,
                wanted,
                Some(&java_info),
                &|l: &str| log(l),
            )
            .await?;
        if !installed.loader_version.is_empty()
            && installed.loader_version != profile.loader_version
        {
            inner.patch_profile(&id, &json!({"loaderVersion": installed.loader_version}))?;
        }

        // 3. Files
        state(LaunchState::Downloading);
        let progress_inner = Arc::clone(inner);
        let progress_id = id.clone();
        let on_progress: mango_minecraft::ProgressCb = Arc::new(move |p: InstallProgress| {
            let phase = match p.phase {
                Phase::Client => ProgressPhase::Client,
                Phase::Libraries => ProgressPhase::Libraries,
                Phase::Assets => ProgressPhase::Assets,
                Phase::Done => ProgressPhase::Done,
            };
            let force = p.phase == Phase::Done || (p.total > 0 && p.done >= p.total);
            emit_progress(
                &progress_inner,
                &progress_id,
                phase,
                p.done as u64,
                p.total as u64,
                &p.label,
                force,
            );
        });
        let log_inner = Arc::clone(inner);
        let log_id = id.clone();
        let install = install_version(
            paths,
            &installed.version_id,
            &InstallOptions {
                concurrency: config.concurrent_downloads.clamp(1, 32) as usize,
                on_progress: Some(on_progress),
                on_log: Some(Arc::new(move |l: &str| {
                    log_inner.emit(Event::LaunchLog(LaunchLogEvent {
                        profile_id: log_id.clone(),
                        line: l.to_string(),
                        level: LogLevelName::Info,
                    }));
                })),
                fetch: inner.opts.fetch.clone(),
                endpoints: mango_minecraft::Endpoints {
                    manifest: inner.opts.manifest_url.clone(),
                    resources: inner.opts.resources_url.clone(),
                },
                ..InstallOptions::default()
            },
        )
        .await?;

        // Loaders can raise the Java requirement.
        let merged = mango_java::required_major_from_json(&install.version);
        let java = if merged > java.major {
            state(LaunchState::Java);
            self.resolve_java_for(&id, &config, profile.java_path.as_deref(), merged)
                .await?
        } else {
            java
        };
        let java_exe = launch_executable(&java.path, Platform::current(), true);

        // 4. Account (refreshed above)
        state(LaunchState::Account);

        // 5. Arguments
        let window = window_for(&profile, &config);
        let mut input = LaunchInput {
            java_path: java_exe,
            paths: paths.clone(),
            version: Value::Null,
            classpath: Vec::new(),
            client_jar: PathBuf::new(),
            natives_dir: PathBuf::new(),
            assets_index_id: String::new(),
            is_legacy_assets: false,
            logging_arg: None,
            game_dir: game_dir.clone(),
            profile_id: id.clone(),
            profile_name: profile.name.clone(),
            ram_mb: profile.effective_ram_mb(config.ram),
            preset: Preset::from_name(&config.performance_preset),
            global_java_args: config.java_args.clone(),
            profile_java_args: profile.java_args.clone(),
            fullscreen: window.0,
            width: window.1,
            height: window.2,
            quick_join,
            account: creds.to_launch_account(),
            launcher_version: inner.opts.app_version.clone(),
            env: HostEnv::current(),
        };
        input.apply_install(&install);
        let command = build_launch_command(&input);

        // 6. Go
        state(LaunchState::Launching);
        let log_file = log_file_for(&paths.logs(), &profile.name, &id);
        let weak = Arc::downgrade(inner);
        let cb_id = id.clone();
        let cb: EventCb = Arc::new(move |e| handle_game_event(&weak, &cb_id, e));
        let game = GameInstance::spawn(SpawnOptions::new(
            command,
            log_file,
            &id,
            &installed.version_id,
            cb,
        ))?;
        let session = game.session();
        lock(&inner.games).insert(id.clone(), game.clone());
        inner.patch_profile(
            &id,
            &json!({"session": {"pid": session.pid, "versionId": session.version_id, "startedAt": session.started_at}}),
        )?;
        // A game that died instantly may have run its exit handler before the
        // session was written: undo that stale record.
        if game.has_exited() {
            lock(&inner.games).remove(&id);
            let _ = inner.patch_profile(&id, &json!({"session": null}));
        }
        Ok(LaunchResult {
            pid: session.pid,
            version_id: session.version_id,
        })
    }

    pub(crate) async fn resolve_java_for(
        &self,
        profile_id: &str,
        config: &Config,
        profile_java: Option<&str>,
        required: u32,
    ) -> Res<mango_java::Resolved> {
        let mut opts = self.java_options();
        opts.custom_path = crate::java_cmds::custom_java_path(profile_java, &config.java_path);
        let inner = Arc::clone(&self.inner);
        let id = profile_id.to_string();
        opts.progress = Some(Arc::new(move |done, total| {
            let force = total > 0 && done >= total;
            emit_progress(&inner, &id, ProgressPhase::Java, done, total, "", force);
        }));
        Ok(resolve_java(required, &opts).await?)
    }

    /// Re-attach to games started before this run; clear dead sessions.
    pub(crate) fn adopt_sessions(&self) {
        let inner = &self.inner;
        let sessions: Vec<(String, String, mango_core::Session)> = inner.with_store(|s| {
            s.profiles
                .iter()
                .filter_map(|p| {
                    p.session
                        .clone()
                        .map(|ses| (p.id.clone(), p.name.clone(), ses))
                })
                .collect()
        });
        for (id, name, session) in sessions {
            let weak = Arc::downgrade(inner);
            let cb_id = id.clone();
            let cb: EventCb = Arc::new(move |e| handle_game_event(&weak, &cb_id, e));
            let log_file = log_file_for(&inner.paths.logs(), &name, &id);
            match GameInstance::adopt(&session, AdoptOptions::new(&id, log_file, cb)) {
                Some(game) => {
                    lock(&inner.games).insert(id.clone(), game);
                    inner.set_state(&id, LaunchState::Running);
                }
                None => {
                    let _ = inner.patch_profile(&id, &json!({"session": null}));
                }
            }
        }
    }
}

pub(crate) fn emit_progress(
    inner: &Inner,
    profile_id: &str,
    phase: ProgressPhase,
    done: u64,
    total: u64,
    label: &str,
    force: bool,
) {
    if lock(&inner.throttle).ready(profile_id, force, Instant::now()) {
        inner.emit(Event::LaunchProgress(LaunchProgressEvent {
            profile_id: profile_id.to_string(),
            phase,
            done,
            total,
            label: label.to_string(),
        }));
    }
}

/// Runs on the runtime's follower threads.
pub(crate) fn handle_game_event(weak: &Weak<Inner>, profile_id: &str, event: GameEvent) {
    let Some(inner) = weak.upgrade() else { return };
    match event {
        GameEvent::Log { line, level } => {
            let level = match level {
                LogLevel::Error => LogLevelName::Error,
                LogLevel::Info if line.contains("/WARN]") => LogLevelName::Warn,
                LogLevel::Info => LogLevelName::Info,
            };
            inner.emit(Event::LaunchLog(LaunchLogEvent {
                profile_id: profile_id.to_string(),
                line,
                level,
            }));
        }
        GameEvent::Running => inner.set_state(profile_id, LaunchState::Running),
        GameEvent::Exited(info) => {
            lock(&inner.games).remove(profile_id);
            let played = info.played_ms.unwrap_or(0);
            let saved = inner.with_store(|s| {
                let total = s.get_profile(profile_id).map_or(0, |p| p.play_time_ms) + played;
                s.update_profile(
                    profile_id,
                    &json!({"session": null, "lastPlayed": now_ms(), "playTimeMs": total}),
                )
                .map(|_| ())
            });
            if let Err(e) = saved {
                tracing::warn!("could not save play time for {profile_id}: {e}");
            }
            if played > 0 {
                let _ = mango_mango::stats::Stats::new(&inner.paths)
                    .record_ending_now(profile_id, played);
            }
            let state = match info.state {
                GameState::Crashed => LaunchState::Crashed,
                _ => LaunchState::Stopped,
            };
            inner.set_state(profile_id, state);
        }
    }
}

/// `(fullscreen, width, height)` of a launch: the instance's own window
/// setting, else the global config. Vanilla has no "start maximized" flag, so
/// there is no such mode.
pub(crate) fn window_for(profile: &Profile, config: &Config) -> (bool, u32, u32) {
    use mango_core::WindowMode;
    match profile.window {
        Some(w) if w.mode == WindowMode::Fullscreen => (true, config.width, config.height),
        Some(w) if w.mode == WindowMode::Size => (
            false,
            w.width.unwrap_or(config.width),
            w.height.unwrap_or(config.height),
        ),
        _ => (config.fullscreen, config.width, config.height),
    }
}

/// Mirror the language into a fresh options.txt without touching an existing one.
fn seed_options(game_dir: &Path, config: &Config) -> std::io::Result<()> {
    let file = game_dir.join("options.txt");
    if file.exists() {
        return Ok(());
    }
    std::fs::create_dir_all(game_dir)?;
    let lang = if config.language == "de" {
        "de_de"
    } else {
        "en_us"
    };
    std::fs::write(
        file,
        format!(
            "lang:{lang}\nguiScale:0\nmaxFps:260\nenableVsync:false\nrenderDistance:12\n\
             soundCategory_master:0.5\nautoJump:false\ntoggleCrouch:false\npauseOnLostFocus:false\n"
        ),
    )
}

struct ModrinthPerf(mango_modrinth::Client);

impl ModrinthLookup for ModrinthPerf {
    async fn resolve(
        &self,
        slug: &str,
        mc_version: &str,
    ) -> Result<Option<ResolvedFile>, ContentError> {
        let versions = self
            .0
            .versions(slug, Some("fabric"), Some(mc_version))
            .await
            .map_err(|e| ContentError::Lookup(e.to_string()))?;
        Ok(mango_modrinth::pick_version(&versions)
            .and_then(mango_modrinth::primary_file)
            .map(|f| ResolvedFile {
                filename: f.filename.clone(),
                url: f.url.clone(),
                sha512: f.hashes.sha512.clone(),
                sha1: f.hashes.sha1.clone(),
                size: f.size,
            }))
    }
}

trait LoaderStr {
    fn loader_str(&self) -> &'static str;
}

impl LoaderStr for Profile {
    fn loader_str(&self) -> &'static str {
        use mango_core::Loader as L;
        match self.loader {
            L::Vanilla => "vanilla",
            L::Fabric => "fabric",
            L::Quilt => "quilt",
            L::Forge => "forge",
            L::Neoforge => "neoforge",
        }
    }
}
