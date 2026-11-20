//! `mango-cli launch`: the real pipeline in port-spec launch order.

use crate::args::Args;
use crate::commands::open_store;
use crate::error::{fail, usage, CliError, Res};
use mango_content::{ContentError, ModrinthLookup, PerfOptions, ResolvedFile};
use mango_core::model::Loader as CoreLoader;
use mango_core::{Config, Paths, Profile, Store};
use mango_java::{launch_executable, resolve_java, JavaInfo, Platform, ResolveOptions};
use mango_loaders::{Loader, Loaders};
use mango_mango::mangoconfig::{self, Bundle};
use mango_minecraft::{
    build_launch_command, install_version, resolve_version, HostEnv, InstallOptions,
    InstallProgress, LaunchCommand, LaunchInput, Phase, Preset, VERSION_MANIFEST_URL,
};
use mango_net::FetchOptions;
use mango_runtime::{
    is_session_alive, log_file_for, EventCb, GameEvent, GameInstance, GameState, LogLevel,
    SpawnOptions,
};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

struct Opts {
    target: String,
    stop_when_running: bool,
    timeout: Option<Duration>,
    dry_run: bool,
    perf_mods: bool,
    auto_fabric: bool,
    quick_join: Option<String>,
    /// Account name or uuid (default: the selected one).
    account: Option<String>,
}

fn parse(mut args: Args) -> Res<Opts> {
    let stop_when_running = args.flag("--stop-when-running");
    let dry_run = args.flag("--dry-run");
    let perf_mods = args.flag("--perf-mods");
    let auto_fabric = args.flag("--auto-fabric");
    let timeout = args.number("--timeout-secs")?.map(Duration::from_secs);
    let quick_join = args.value("--quick-join")?;
    let account = args.value("--account")?;
    let target = args.positional();
    args.finish()?;
    let Some(target) = target else {
        return usage("usage: launch <profile-name-or-id> [--stop-when-running] [--timeout-secs N] [--dry-run]");
    };
    Ok(Opts {
        target,
        stop_when_running,
        timeout,
        dry_run,
        perf_mods,
        auto_fabric,
        quick_join,
        account,
    })
}

fn stage(name: &str) {
    println!("STAGE {name}");
}

/// Hide secrets from anything printed.
struct Redactor {
    token: String,
}

impl Redactor {
    fn text(&self, s: &str) -> String {
        // Short tokens (offline "0") would shred ordinary text; the argument
        // rules in `args` cover those.
        if self.token.len() >= 8 {
            s.replace(&self.token, "<redacted>")
        } else {
            s.to_string()
        }
    }

    fn args(&self, args: &[String]) -> Vec<String> {
        let mut out = Vec::with_capacity(args.len());
        let mut hide_next = false;
        for a in args {
            if hide_next {
                out.push("<redacted>".to_string());
                hide_next = false;
            } else if a == "--accessToken" {
                out.push(a.clone());
                hide_next = true;
            } else if a.starts_with("token:") {
                out.push("token:<redacted>".to_string());
            } else {
                out.push(self.text(a));
            }
        }
        out
    }
}

/// Rate limiter for progress lines (~10/s).
struct Throttle {
    last: Mutex<Option<Instant>>,
}

impl Throttle {
    fn new() -> Self {
        Self {
            last: Mutex::new(None),
        }
    }

    fn ready(&self, force: bool) -> bool {
        let Ok(mut last) = self.last.lock() else {
            return true;
        };
        let now = Instant::now();
        let due = force || last.is_none_or(|t| now.duration_since(t) >= Duration::from_millis(100));
        if due {
            *last = Some(now);
        }
        due
    }
}

fn find_profile<'a>(store: &'a Store, target: &str) -> Res<&'a Profile> {
    if let Some(p) = store.get_profile(target) {
        return Ok(p);
    }
    let lower = target.to_lowercase();
    let matches: Vec<&Profile> = store
        .profiles
        .iter()
        .filter(|p| p.name.to_lowercase() == lower)
        .collect();
    match matches.as_slice() {
        [one] => Ok(one),
        [] => Err(CliError::Failure(format!(
            "no profile named {target:?} (see `profile list`)"
        ))),
        _ => Err(CliError::Failure(format!(
            "several profiles are named {target:?}; use the id"
        ))),
    }
}

fn loader_str(l: CoreLoader) -> &'static str {
    match l {
        CoreLoader::Vanilla => "vanilla",
        CoreLoader::Fabric => "fabric",
        CoreLoader::Quilt => "quilt",
        CoreLoader::Forge => "forge",
        CoreLoader::Neoforge => "neoforge",
    }
}

fn bundle_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("MANGO_BUNDLE_DIR").filter(|d| !d.is_empty()) {
        return PathBuf::from(dir);
    }
    // crates/cli -> crates -> repo root
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("bundle/mangoconfig")
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
        let file = mango_modrinth::pick_version(&versions)
            .and_then(mango_modrinth::primary_file)
            .map(|f| ResolvedFile {
                filename: f.filename.clone(),
                url: f.url.clone(),
                sha512: f.hashes.sha512.clone(),
                sha1: f.hashes.sha1.clone(),
                size: f.size,
            });
        Ok(file)
    }
}

fn java_progress() -> mango_java::ProgressFn {
    let throttle = Throttle::new();
    Arc::new(move |done, total| {
        if throttle.ready(total > 0 && done >= total) {
            let mb = |b: u64| b as f64 / 1_048_576.0;
            if total > 0 {
                println!("[java] downloading {:.1}/{:.1} MB", mb(done), mb(total));
            } else {
                println!("[java] downloading {:.1} MB", mb(done));
            }
        }
    })
}

fn install_progress() -> mango_minecraft::ProgressCb {
    let throttle = Throttle::new();
    Arc::new(move |p: InstallProgress| {
        let phase = match p.phase {
            Phase::Client => "client",
            Phase::Libraries => "libraries",
            Phase::Assets => "assets",
            Phase::Done => "done",
        };
        let force = p.phase == Phase::Done || (p.total > 0 && p.done >= p.total);
        if throttle.ready(force) {
            println!("[downloading] {phase} {}/{} {}", p.done, p.total, p.label);
        }
    })
}

async fn resolve_java_for(
    paths: &Paths,
    config: &Config,
    required: u32,
) -> Res<mango_java::Resolved> {
    let mut opts = ResolveOptions::new(paths.runtimes(), paths.cache());
    if !config.java_path.is_empty() {
        opts.custom_path = Some(PathBuf::from(&config.java_path));
    }
    opts.progress = Some(java_progress());
    resolve_java(required, &opts)
        .await
        .map_err(fail("resolving Java"))
}

pub async fn run(paths: &Paths, args: Args) -> Res<()> {
    let opts = parse(args)?;
    // Account first: refreshing a Microsoft token rewrites accounts.json, so
    // this must finish before the store below is opened (it would otherwise
    // write its stale copy back over the rotated refresh token).
    stage("preparing");
    let creds = crate::account::launch_credentials(paths, opts.account.as_deref()).await?;
    let mut store = open_store(paths)?;
    for w in &store.warnings {
        eprintln!("warning: {w}");
    }
    let mut profile = find_profile(&store, &opts.target)?.clone();
    let config = store.config.clone();
    let redact = Redactor {
        token: creds.access_token().to_string(),
    };

    // Reject when a session is still alive.
    if let Some(s) = &profile.session {
        if is_session_alive(s) {
            return Err(CliError::Failure(format!(
                "{:?} is already running (pid {})",
                profile.name, s.pid
            )));
        }
        store
            .update_profile(&profile.id, &json!({"session": null}))
            .map_err(fail("clearing the stale session"))?;
        profile.session = None;
    }
    println!(
        "Preparing {} (Minecraft {}, {})",
        profile.name,
        profile.mc_version,
        loader_str(profile.loader)
    );

    let bundle_path = bundle_dir();
    // Vanilla auto-switches to Fabric in the legacy launcher; opt-in here so a
    // vanilla profile can be verified as vanilla.
    if opts.auto_fabric {
        let bundle = Bundle::scan(&bundle_path);
        if mangoconfig::should_switch_vanilla_to_fabric(&profile, &config, &bundle) {
            println!("Switching vanilla profile to Fabric for MangoConfig");
            store
                .update_profile(
                    &profile.id,
                    &json!({"loader": "fabric", "loaderVersion": ""}),
                )
                .map_err(fail("switching to Fabric"))?;
            profile = find_profile(&store, &profile.id)?.clone();
        }
    }

    // MangoConfig and the performance mods never throw or block a launch.
    let outcome =
        mangoconfig::ensure_with_log(paths, &profile, &config, &bundle_path, &|l| println!("{l}"));
    println!("MangoConfig: {outcome:?}");
    if opts.perf_mods {
        let lookup = ModrinthPerf(mango_modrinth::Client::with_base(mango_modrinth::API));
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
            println!("{l}");
        }
        println!(
            "Performance mods: {:?} installed={:?} skipped={:?}",
            perf.state, perf.installed, perf.skipped
        );
    }

    let game_dir = paths
        .instance_dir(&profile.id)
        .map_err(fail("locating the instance"))?;
    if let Err(e) = seed_options(&game_dir, &config) {
        eprintln!("warning: could not seed options.txt: {e}");
    }
    let icons = mango_servers::ServerIcons::new(paths.server_icons_file());
    if let Err(e) =
        mango_servers::seed_featured_servers(&game_dir, &mango_servers::featured_servers(), &icons)
            .await
    {
        println!("Partnered server list skipped: {e}");
    }

    // 1. Java against the vanilla metadata (installers need a JVM first).
    stage("java");
    let fetch = FetchOptions::default();
    let base = resolve_version(paths, &profile.mc_version, VERSION_MANIFEST_URL, &fetch)
        .await
        .map_err(fail("resolving the Minecraft version"))?;
    let required = mango_java::required_major_from_json(&base.version);
    let java = resolve_java_for(paths, &config, required).await?;
    println!(
        "Java {} ({:?}): {}",
        java.major,
        java.source,
        java.path.display()
    );

    // 2. Loader
    stage("loader");
    let loader = Loader::parse(loader_str(profile.loader))
        .ok_or_else(|| CliError::Failure("unsupported loader".into()))?;
    let java_info = JavaInfo {
        path: java.path.clone(),
        major: java.major,
        raw: String::new(),
    };
    let wanted_lv = (!profile.loader_version.is_empty()).then_some(profile.loader_version.as_str());
    let installed = Loaders::default()
        .ensure(
            paths,
            loader,
            &profile.mc_version,
            wanted_lv,
            Some(&java_info),
            &|l: &str| println!("{l}"),
        )
        .await
        .map_err(fail("installing the loader"))?;
    if !installed.loader_version.is_empty() && installed.loader_version != profile.loader_version {
        store
            .update_profile(
                &profile.id,
                &json!({"loaderVersion": installed.loader_version}),
            )
            .map_err(fail("saving the loader version"))?;
    }

    // 3. Files
    stage("downloading");
    let install = install_version(
        paths,
        &installed.version_id,
        &InstallOptions {
            concurrency: config.concurrent_downloads.clamp(1, 32) as usize,
            on_progress: Some(install_progress()),
            on_log: Some(Arc::new(|l: &str| println!("{l}"))),
            ..InstallOptions::default()
        },
    )
    .await
    .map_err(fail("installing Minecraft"))?;

    // Loaders can raise the Java requirement: re-check on the merged version.
    let merged_required = mango_java::required_major_from_json(&install.version);
    let java = if merged_required > java.major {
        stage("java");
        let upgraded = resolve_java_for(paths, &config, merged_required).await?;
        println!(
            "Switched to Java {} for {}",
            upgraded.major, installed.version_id
        );
        upgraded
    } else {
        java
    };
    let platform = Platform::current();
    let java_exe = launch_executable(&java.path, platform, true);

    // 4. Account (refreshed above)
    stage("account");

    // 5. Arguments
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
        profile_id: profile.id.clone(),
        profile_name: profile.name.clone(),
        ram_mb: profile.ram.unwrap_or(config.ram),
        preset: Preset::from_name(&config.performance_preset),
        global_java_args: config.java_args.clone(),
        profile_java_args: profile.java_args.clone(),
        fullscreen: config.fullscreen,
        width: config.width,
        height: config.height,
        quick_join: opts.quick_join.clone(),
        account: creds.to_launch_account(),
        launcher_version: env!("CARGO_PKG_VERSION").to_string(),
        env: HostEnv::current(),
    };
    input.apply_install(&install);
    let command = build_launch_command(&input);

    // 6. Go
    stage("launching");
    if opts.dry_run {
        print_command(&command, &redact);
        println!("RESULT dry-run ok");
        return Ok(());
    }
    run_game(
        paths,
        &mut store,
        &profile,
        &installed.version_id,
        command,
        &redact,
        &opts,
    )
}

fn print_command(cmd: &LaunchCommand, redact: &Redactor) {
    println!("cwd: {}", cmd.cwd.display());
    println!("program: {}", cmd.program.display());
    for (k, v) in &cmd.env {
        println!("env: {k}={v}");
    }
    for a in redact.args(&cmd.args) {
        println!("arg: {a}");
    }
}

fn run_game(
    paths: &Paths,
    store: &mut Store,
    profile: &Profile,
    version_id: &str,
    command: LaunchCommand,
    redact: &Redactor,
    opts: &Opts,
) -> Res<()> {
    let (tx, rx) = mpsc::channel::<GameEvent>();
    let cb: EventCb = Arc::new(move |e| {
        let _ = tx.send(e);
    });
    let log_file = log_file_for(&paths.logs(), &profile.name, &profile.id);
    let spawn = SpawnOptions::new(command, log_file.clone(), &profile.id, version_id, cb);
    let game = GameInstance::spawn(spawn).map_err(fail("starting the game"))?;
    let started = Instant::now();
    println!(
        "RESULT spawned pid={} log={}",
        game.pid(),
        log_file.display()
    );
    let session = game.session();
    store
        .update_profile(
            &profile.id,
            &json!({"session": {"pid": session.pid, "versionId": session.version_id, "startedAt": session.started_at}}),
        )
        .map_err(fail("saving the session"))?;

    let stop_sent = AtomicBool::new(false);
    let mut timed_out = false;
    loop {
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(GameEvent::Log { line, level }) => {
                let line = redact.text(&line);
                match level {
                    LogLevel::Error => println!("[err] {line}"),
                    LogLevel::Info => println!("{line}"),
                }
            }
            Ok(GameEvent::Running) => {
                stage("running");
                println!(
                    "RESULT running pid={} after={:.1}s",
                    game.pid(),
                    started.elapsed().as_secs_f64()
                );
                if opts.stop_when_running && !stop_sent.swap(true, Ordering::SeqCst) {
                    game.stop().map_err(fail("stopping the game"))?;
                }
            }
            Ok(GameEvent::Exited(info)) => {
                let played = info.played_ms.unwrap_or(0);
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(0, |d| d.as_millis() as u64);
                let total = store.get_profile(&profile.id).map_or(0, |p| p.play_time_ms) + played;
                if let Err(e) = store.update_profile(
                    &profile.id,
                    &json!({"session": null, "lastPlayed": now, "playTimeMs": total}),
                ) {
                    eprintln!("warning: could not save play time: {e}");
                }
                let _ =
                    mango_mango::stats::Stats::new(paths).record_ending_now(&profile.id, played);
                let code = info
                    .code
                    .map_or_else(|| "signal".to_string(), |c| c.to_string());
                return match info.state {
                    GameState::Crashed => {
                        println!("RESULT crashed code={code}");
                        Err(CliError::Crashed(code))
                    }
                    _ if timed_out => {
                        println!("RESULT stopped code={code}");
                        Err(CliError::Timeout(format!(
                            "timed out after {}s and stopped the game",
                            opts.timeout.map_or(0, |t| t.as_secs())
                        )))
                    }
                    _ => {
                        println!("RESULT stopped code={code}");
                        Ok(())
                    }
                };
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                return Err(CliError::Failure("lost the game event stream".into()));
            }
        }
        if let Some(limit) = opts.timeout {
            if !timed_out && started.elapsed() > limit {
                timed_out = true;
                println!("RESULT timeout after={}s", limit.as_secs());
                if !stop_sent.swap(true, Ordering::SeqCst) {
                    game.stop().map_err(fail("stopping the game"))?;
                }
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn redacts_tokens_in_args_and_text() {
        let r = Redactor {
            token: "eyJhbGciOiJSUzI1NiJ9".into(),
        };
        let args: Vec<String> = [
            "--accessToken",
            "eyJhbGciOiJSUzI1NiJ9",
            "--session",
            "token:abc:u",
            "-Dx=eyJhbGciOiJSUzI1NiJ9",
        ]
        .map(String::from)
        .to_vec();
        let out = r.args(&args).join(" ");
        assert!(!out.contains("eyJ"));
        assert!(out.contains("token:<redacted>"));
        // Short offline tokens only get argument-position redaction.
        let off = Redactor { token: "0".into() };
        assert_eq!(off.text("a0b"), "a0b");
        assert_eq!(
            off.args(&["--accessToken".into(), "0".into()])[1],
            "<redacted>"
        );
    }

    #[test]
    fn throttle_limits_rate_but_honours_force() {
        let t = Throttle::new();
        assert!(t.ready(false));
        assert!(!t.ready(false));
        assert!(t.ready(true));
    }

    #[test]
    fn finds_profiles_by_name_or_id() {
        let tmp = tempfile::tempdir().unwrap();
        let mut store = Store::open(Paths::new(tmp.path())).unwrap();
        let p = store
            .add_profile(&json!({"name": "Alpha", "mcVersion": "1.21"}))
            .unwrap();
        assert_eq!(find_profile(&store, &p.id).unwrap().name, "Alpha");
        assert_eq!(find_profile(&store, "alpha").unwrap().id, p.id);
        assert!(find_profile(&store, "nope").is_err());
    }
}
