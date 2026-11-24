//! The command surface of `docs/ipc.md`.

use crate::accounts::Session;
use crate::error::{AppError, Res};
use crate::events::{Event, EventSink, LaunchStateEvent, ProgressThrottle};
use crate::host::{Host, LoginWindow};
use crate::path_grants::GrantKind;
use crate::types::{
    installed_versions, AppState, ConfigView, Latest, LaunchState, LoaderVersion, ProfileView,
    PublicAccount, VersionEntry, VersionManifest,
};
use mango_auth::Vault;
use mango_core::{validate_profile_id, Paths, Profile, Store};
use mango_minecraft::{get_version_manifest, VERSION_MANIFEST_URL};
use mango_net::FetchOptions;
use mango_runtime::GameInstance;
use mango_servers::{PingOptions, ServerIcons};
use mango_updater::{Updater, UpdaterConfig};
use serde_json::{json, Map, Value};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

pub const DEFAULT_UPDATE_URL: &str =
    "https://github.com/MangoMax3537/mangoclient/releases/latest/download/latest.json";

/// Everything that points outside the data dir, injectable for tests.
#[derive(Debug, Clone)]
pub struct ServiceOptions {
    /// Directory holding the bundled `MangoConfig-*.jar` files.
    pub bundle_dir: PathBuf,
    pub manifest_url: String,
    pub loader_endpoints: mango_loaders::Endpoints,
    pub app_version: String,
    /// Really launch the OS file manager in `open_folder`.
    pub open_folders: bool,
    /// Modrinth API base (no trailing slash).
    pub modrinth_api: String,
    /// mclo.gs log upload endpoint.
    pub mclogs_api: String,
    /// Retry/timeout policy for the service's own HTTP calls (also used by
    /// the launch pipeline).
    pub fetch: FetchOptions,
    /// Minecraft assets (`resources.download.minecraft.net`) base URL.
    pub resources_url: String,
    /// Adoptium API base for the automatic Java download.
    pub adoptium_base: String,
    /// Replaces the built-in host allow-lists for modpack files and the
    /// Adoptium download. `None` = production lists. Tests with local mock
    /// servers use `Some(HostPolicy::allow_all())`.
    pub download_policy: Option<mango_net::HostPolicy>,
    /// Server list ping settings.
    pub ping: PingOptions,
    /// Partner servers, shown first and starred.
    pub partners: Vec<mango_servers::FeaturedServer>,
    /// `latest.json` of the updater.
    pub update_manifest_url: String,
    /// Unpackaged dev run: the updater reports `disabled`.
    pub update_dev: bool,
    /// Base64 Ed25519 key updates must be signed with (tests: their own key).
    pub update_public_key: String,
    /// Pretend to be another platform (tests). `None` = detect.
    pub update_platform: Option<mango_updater::Platform>,
    /// Which URLs an update may come from. `None` = GitHub over https.
    pub update_policy: Option<mango_updater::UrlPolicy>,
    /// Pause between a verified launch-time update and handing it to the
    /// installer, so the UI can paint.
    pub update_relaunch_delay: Duration,
    /// Check for updates at startup and every 3 h (`start_background`).
    pub auto_update_check: bool,
    /// Telemetry base, already validated. `None` = `MANGO_API_BASE` / legacy.
    pub api_base: Option<String>,
    /// How often the `mods/` watcher looks at the selected/running profiles.
    pub watch_interval: Duration,
    /// Hosts remote images (Modrinth icons, gallery) may be fetched from.
    pub icon_hosts: Vec<String>,
    /// Allow plain http for remote images (tests only).
    pub icon_allow_http: bool,
    /// Microsoft / Xbox / Minecraft services endpoints.
    pub auth_endpoints: mango_auth::Endpoints,
    /// Where system JVMs are looked for. `None` = PATH, JAVA_HOME and the
    /// usual JVM folders (tests point this at a temp dir).
    pub java_roots: Option<mango_java::SearchRoots>,
    /// Pause before the startup skin fetch of each Microsoft account (the
    /// fetches run one after the other and never block startup).
    pub skin_prefetch_delay: Duration,
    /// Path-taking commands (import, export, picture, watch folder) accept
    /// only paths a `pick_*` command returned recently. On in the Tauri
    /// shell, off in the dev server and tests (the browser flow types paths).
    pub strict_paths: bool,
    /// How long a picked path stays usable.
    pub path_grant_ttl: Duration,
}

impl Default for ServiceOptions {
    fn default() -> Self {
        ServiceOptions {
            bundle_dir: resolve_bundle_dir(std::env::var_os("MANGO_BUNDLE_DIR"), None),
            manifest_url: VERSION_MANIFEST_URL.to_string(),
            loader_endpoints: mango_loaders::Endpoints::default(),
            app_version: env!("CARGO_PKG_VERSION").to_string(),
            open_folders: true,
            modrinth_api: mango_modrinth::API.to_string(),
            mclogs_api: "https://api.mclo.gs/1/log".to_string(),
            fetch: FetchOptions::default(),
            resources_url: mango_minecraft::RESOURCES_URL.to_string(),
            adoptium_base: mango_java::DEFAULT_ADOPTIUM_BASE.to_string(),
            download_policy: None,
            ping: PingOptions::default(),
            partners: mango_servers::featured_servers(),
            update_manifest_url: std::env::var("MANGO_UPDATE_URL")
                .ok()
                .filter(|u| !u.is_empty())
                .unwrap_or_else(|| DEFAULT_UPDATE_URL.to_string()),
            update_dev: cfg!(debug_assertions),
            update_public_key: mango_updater::UPDATE_PUBLIC_KEY_B64.to_string(),
            update_platform: None,
            update_policy: None,
            update_relaunch_delay: Duration::from_millis(800),
            auto_update_check: true,
            api_base: None,
            watch_interval: Duration::from_secs(2),
            icon_hosts: vec!["cdn.modrinth.com".to_string()],
            icon_allow_http: false,
            auth_endpoints: mango_auth::Endpoints::default(),
            java_roots: None,
            skin_prefetch_delay: Duration::from_millis(1500),
            strict_paths: false,
            path_grant_ttl: crate::path_grants::DEFAULT_GRANT_TTL,
        }
    }
}

impl ServiceOptions {
    /// Point `bundle_dir` at the packaged resources: `MANGO_BUNDLE_DIR` still
    /// wins, then `<resource_dir>/mangoconfig`, then the dev checkout.
    #[must_use]
    pub fn with_resource_dir(mut self, resource_dir: &Path) -> Self {
        self.bundle_dir =
            resolve_bundle_dir(std::env::var_os("MANGO_BUNDLE_DIR"), Some(resource_dir));
        self
    }
}

/// Where the MangoConfig jars live: an explicit `env_dir` (non-empty), else
/// `<resource_dir>/mangoconfig` if that directory exists, else the
/// development checkout (crates/app -> crates -> repo root).
pub fn resolve_bundle_dir(
    env_dir: Option<std::ffi::OsString>,
    resource_dir: Option<&Path>,
) -> PathBuf {
    if let Some(dir) = env_dir.filter(|d| !d.is_empty()) {
        return PathBuf::from(dir);
    }
    if let Some(packaged) = resource_dir.map(|r| r.join("mangoconfig")) {
        if packaged.is_dir() {
            return packaged;
        }
    }
    dev_bundle_dir()
}

/// The checkout's `bundle/mangoconfig`. Debug builds only: a release build
/// must never point at the build machine's source tree.
#[cfg(debug_assertions)]
fn dev_bundle_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("bundle/mangoconfig")
}

/// Release: no bundle (nothing is injected) rather than a build-machine path.
#[cfg(not(debug_assertions))]
fn dev_bundle_dir() -> PathBuf {
    std::env::temp_dir().join("mangoclient-no-bundle")
}

pub(crate) struct Inner {
    pub paths: Paths,
    pub store: Mutex<Store>,
    pub sink: Arc<dyn EventSink>,
    pub opts: ServiceOptions,
    pub games: Mutex<HashMap<String, GameInstance>>,
    pub launching: Mutex<HashSet<String>>,
    pub states: Mutex<HashMap<String, LaunchState>>,
    pub throttle: Mutex<ProgressThrottle>,
    pub host: Host,
    pub modrinth: mango_modrinth::Client,
    pub icons: Arc<ServerIcons>,
    pub updater: Updater,
    pub telemetry: Mutex<Option<mango_mango::telemetry::Telemetry>>,
    /// Per profile: project id -> newer version number, from the last check.
    pub updates: Mutex<HashMap<String, HashMap<String, String>>>,
    pub vault: Arc<Vault>,
    pub login: Arc<dyn LoginWindow>,
    pub dialog: Arc<dyn crate::host::FileDialog>,
    /// Runtime account state (tokens in memory only), by uuid.
    pub sessions: Mutex<HashMap<String, Session>>,
    /// Serializes token refreshes so a rotated refresh token is never lost.
    pub refresh_gate: tokio::sync::Mutex<()>,
    pub signing_in: AtomicBool,
    /// Installs, imports and restores in flight (see `shell::BusyGuard`).
    pub busy_ops: std::sync::atomic::AtomicUsize,
    /// State of the block-gallery job.
    pub blocks_run: Mutex<crate::block_icons::BlocksRun>,
    /// Profiles with a panorama extraction in flight.
    pub pano_busy: Mutex<HashSet<String>>,
    /// Serializes creating, pruning, restoring and deleting backups.
    pub backup_gate: Mutex<()>,
    /// Paths the user picked in native dialogs (see `path_grants`).
    pub grants: crate::path_grants::PathGrants,
    /// Serializes edits of the local skin library.
    pub skin_gate: Mutex<()>,
    /// Per account: serializes skin refreshes and changes (see `skins::SkinFlight`).
    pub skin_flights: Mutex<HashMap<String, Arc<crate::skins::SkinFlight>>>,
    /// Serializes panorama-pool extraction runs.
    pub pool_busy: Mutex<bool>,
}

pub(crate) fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    // A panic elsewhere must not brick the launcher.
    m.lock().unwrap_or_else(|e| e.into_inner())
}

pub(crate) fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

impl Inner {
    pub(crate) fn emit(&self, event: Event) {
        self.sink.emit(event);
    }

    pub(crate) fn set_state(&self, profile_id: &str, state: LaunchState) {
        lock(&self.states).insert(profile_id.to_string(), state);
        self.emit(Event::LaunchState(LaunchStateEvent {
            profile_id: profile_id.to_string(),
            state,
        }));
    }

    pub(crate) fn patch_profile(&self, id: &str, patch: &Value) -> mango_core::Result<()> {
        self.with_store(|s| s.update_profile(id, patch).map(|_| ()))
    }

    pub(crate) fn with_store<R>(&self, f: impl FnOnce(&mut Store) -> R) -> R {
        f(&mut lock(&self.store))
    }

    /// The UI's profile (never call this inside `with_store`).
    pub(crate) fn view(&self, p: &Profile) -> ProfileView {
        let bundle = self.bundle();
        self.with_store(|s| crate::types::view_of(&s.paths, p, &s.config, &bundle))
    }

    /// The bundled MangoConfig jars.
    pub(crate) fn bundle(&self) -> mango_mango::mangoconfig::Bundle {
        mango_mango::mangoconfig::Bundle::scan(&self.opts.bundle_dir)
    }
}

/// Cheap to clone; all clones share the same state.
#[derive(Clone)]
pub struct AppService {
    pub(crate) inner: Arc<Inner>,
}

impl AppService {
    /// Open the data dir and re-attach to games that outlived the last run.
    pub fn new(paths: Paths, sink: Arc<dyn EventSink>, opts: ServiceOptions) -> Res<AppService> {
        let host = Host::system(opts.open_folders);
        Self::with_host(paths, sink, opts, host)
    }

    /// Like [`AppService::new`] with the shell's folder opener and clipboard.
    pub fn with_host(
        paths: Paths,
        sink: Arc<dyn EventSink>,
        opts: ServiceOptions,
        host: Host,
    ) -> Res<AppService> {
        let store = Store::open(paths.clone())?;
        let mut modrinth =
            mango_modrinth::Client::with_options(&opts.modrinth_api, opts.fetch.clone());
        if let Some(policy) = &opts.download_policy {
            modrinth = modrinth.with_download_policy(policy.clone());
        }
        let icons = Arc::new(ServerIcons::in_root(paths.root()));
        let updater = build_updater(&paths, &opts, host.installer.clone());
        let (host_vault, host_login) = (host.vault.clone(), host.login.clone());
        let host_dialog = host.dialog.clone();
        let grants = crate::path_grants::PathGrants::new(opts.strict_paths, opts.path_grant_ttl);
        let svc = AppService {
            inner: Arc::new(Inner {
                paths,
                store: Mutex::new(store),
                sink,
                opts,
                games: Mutex::new(HashMap::new()),
                launching: Mutex::new(HashSet::new()),
                states: Mutex::new(HashMap::new()),
                throttle: Mutex::new(ProgressThrottle::default()),
                host,
                modrinth,
                icons,
                updater,
                telemetry: Mutex::new(None),
                updates: Mutex::new(HashMap::new()),
                vault: host_vault,
                login: host_login,
                dialog: host_dialog,
                sessions: Mutex::new(HashMap::new()),
                refresh_gate: tokio::sync::Mutex::new(()),
                signing_in: AtomicBool::new(false),
                busy_ops: std::sync::atomic::AtomicUsize::new(0),
                blocks_run: Mutex::new(crate::block_icons::BlocksRun::default()),
                pano_busy: Mutex::new(HashSet::new()),
                backup_gate: Mutex::new(()),
                grants,
                skin_gate: Mutex::new(()),
                skin_flights: Mutex::new(HashMap::new()),
                pool_busy: Mutex::new(false),
            }),
        };
        // Migrates legacy plaintext tokens into envelopes and scrubs them.
        svc.load_accounts();
        svc.adopt_sessions();
        Ok(svc)
    }

    pub fn paths(&self) -> &Paths {
        &self.inner.paths
    }

    pub fn app_state(&self) -> Res<AppState> {
        let inner = &self.inner;
        let bundle = inner.bundle();
        let (config, profiles, records) = inner.with_store(|s| {
            (
                ConfigView::from(&s.config),
                s.profiles
                    .iter()
                    .map(|p| crate::types::view_of(&s.paths, p, &s.config, &bundle))
                    .collect(),
                s.accounts.clone(),
            )
        });
        let total = mango_core::model::total_ram_mb();
        let now = now_ms() as i64;
        let accounts = records
            .iter()
            .map(|a| PublicAccount::from_account(a, now, inner.ingest(a)).versioned(&inner.paths))
            .collect();
        Ok(AppState {
            version: inner.opts.app_version.clone(),
            config,
            profiles,
            accounts,
            total_ram_mb: total,
            default_ram_mb: mango_core::ram::default_ram_mb(total),
            ram_limits: mango_core::ram_limits(total).into(),
            running: self.game_running(),
            root_dir: inner.paths.root().display().to_string(),
            desktop_integration_available: crate::desktop_entry::DesktopIntegration::detect()
                .is_some(),
        })
    }

    pub fn config_set(&self, patch: Value) -> Res<ConfigView> {
        let view = self
            .inner
            .with_store(|s| s.set_config(&patch).map(ConfigView::from))?;
        // Opting in/out takes effect immediately (needs a runtime to spawn on).
        self.sync_telemetry();
        if patch.get("desktopIntegration").is_some() || patch.get("firstRunDone").is_some() {
            self.sync_desktop_integration();
        }
        Ok(view)
    }

    // ---- profiles ----

    pub fn profile_create(
        &self,
        name: String,
        mc_version: String,
        loader: String,
        loader_version: Option<String>,
    ) -> Res<ProfileView> {
        parse_loader(&loader)?;
        let mut data = json!({"name": name, "mcVersion": mc_version, "loader": loader});
        if let Some(lv) = loader_version.filter(|v| !v.is_empty()) {
            data["loaderVersion"] = Value::String(lv);
        }
        let p = self.inner.with_store(|s| s.add_profile(&data))?;
        Ok(self.inner.view(&p))
    }

    /// Applies the user-editable fields of `ProfileView` (`ProfilePatch`).
    /// Derived view fields (`coverUrl`, `modCount`, `playTimeMs` ...) are
    /// ignored so a UI may spread a whole view into the patch; keys that are
    /// immutable (`id`, `created`), have their own command (`icon`,
    /// `background`) or are unknown are rejected with a `validation` error.
    pub async fn profile_update(&self, id: String, patch: Value) -> Res<ProfileView> {
        let mut patch = patch;
        // The one key that needs I/O: the instance's own Java must run.
        if let Some(v) = patch.get("javaPath").cloned() {
            let checked = self.check_java_path(&v).await?;
            patch["javaPath"] = checked;
        }
        self.profile_update_checked(id, patch)
    }

    fn profile_update_checked(&self, id: String, patch: Value) -> Res<ProfileView> {
        validate_profile_id(&id)?;
        let Some(obj) = patch.as_object() else {
            return Err(AppError::new("validation", "Patch must be an object"));
        };
        let mut clean = Map::new();
        for (key, v) in obj {
            match key.as_str() {
                "name" | "mcVersion" | "color" | "ram" | "javaArgs" | "mangoConfig" | "group"
                | "tags" | "javaPath" | "window" => {
                    clean.insert(key.clone(), v.clone());
                }
                "loader" => {
                    if let Some(l) = v.as_str() {
                        parse_loader(l)?;
                    }
                    clean.insert(key.clone(), v.clone());
                }
                "loaderVersion" => {
                    // The UI uses null where the store uses "".
                    let v = if v.is_null() { json!("") } else { v.clone() };
                    clean.insert(key.clone(), v);
                }
                "coverUrl" | "lastPlayed" | "playTimeMs" | "modCount" | "autoBlock"
                | "pendingBlocked" // removed field; a stale spread view may still carry it
                | "iconSource" | "ramSource" | "ramHint" | "effectiveRamMb" => {}
                "id" | "created" => {
                    return Err(AppError::new(
                        "validation",
                        format!("Profile field '{key}' is immutable"),
                    ));
                }
                "icon" | "background" => {
                    return Err(AppError::new(
                        "validation",
                        format!("Profile field '{key}' has its own command"),
                    ));
                }
                other => {
                    let shown: String = other.chars().take(40).collect();
                    return Err(AppError::new(
                        "validation",
                        format!("Unknown profile field '{shown}'"),
                    ));
                }
            }
        }
        let bundle = self.inner.bundle();
        self.inner.with_store(|s| {
            // A changed `ram` is the user's choice; `null` goes back to the
            // default. An unchanged value (a spread view) keeps its source.
            if let Some(ram) = clean.get("ram") {
                let current = s.get_profile(&id).ok_or_else(AppError::profile_not_found)?;
                if ram.as_u64() != current.ram.map(u64::from) {
                    let source = if ram.is_null() {
                        Value::Null
                    } else {
                        json!("user")
                    };
                    clean.insert("ramSource".into(), source);
                }
            }
            let p = s
                .update_profile(&id, &Value::Object(clean))?
                .cloned()
                .ok_or_else(AppError::profile_not_found)?;
            Ok(crate::types::view_of(&s.paths, &p, &s.config, &bundle))
        })
    }

    pub fn profile_delete(&self, id: String) -> Res<()> {
        validate_profile_id(&id)?;
        if self.is_busy(&id) {
            return Err(AppError::already_running());
        }
        self.inner.with_store(|s| {
            s.get_profile(&id).ok_or_else(AppError::profile_not_found)?;
            s.delete_profile(&id).map_err(AppError::from)
        })
    }

    pub fn profile_select(&self, id: String) -> Res<ConfigView> {
        validate_profile_id(&id)?;
        self.inner.with_store(|s| {
            s.get_profile(&id).ok_or_else(AppError::profile_not_found)?;
            Ok(ConfigView::from(
                s.set_config(&json!({"selectedProfile": id}))?,
            ))
        })
    }

    pub async fn profile_duplicate(&self, id: String) -> Res<ProfileView> {
        validate_profile_id(&id)?;
        let src = self
            .inner
            .with_store(|s| s.get_profile(&id).cloned())
            .ok_or_else(AppError::profile_not_found)?;
        let mut data = json!({
            "name": format!("{} (copy)", src.name),
            "mcVersion": src.mc_version,
            "loader": src.loader,
            "loaderVersion": src.loader_version,
            "color": src.color,
            "ram": src.ram,
            "javaArgs": src.java_args,
            "mangoConfig": src.mango_config,
            "group": src.group,
            "tags": src.tags,
            "javaPath": src.java_path,
            "window": src.window,
        });
        if data["name"]
            .as_str()
            .is_some_and(|n| n.chars().count() > 128)
        {
            data["name"] = json!(src.name);
        }
        let created = self.inner.with_store(|s| s.add_profile(&data))?;

        let paths = self.inner.paths.clone();
        let (old_id, new_id) = (src.id.clone(), created.id.clone());
        let mods = src.mods.clone();
        let rebased = tokio::task::spawn_blocking(move || -> Res<Vec<mango_core::ModRecord>> {
            let from = paths.instance_dir(&old_id)?;
            let to = paths.instance_dir(&new_id)?;
            if from.is_dir() {
                copy_dir(&from, &to)?;
            }
            if let (Ok(c_from), Ok(c_to)) = (paths.cover_file(&old_id), paths.cover_file(&new_id)) {
                if c_from.is_file() {
                    let _ = std::fs::copy(c_from, c_to);
                }
            }
            Ok(mods
                .into_iter()
                .map(|mut m| {
                    if let Some(file) = m.file.as_deref() {
                        if let Ok(rel) = Path::new(file).strip_prefix(&from) {
                            m.file = Some(to.join(rel).display().to_string());
                        }
                    }
                    m
                })
                .collect())
        })
        .await??;

        let bundle = self.inner.bundle();
        self.inner.with_store(|s| {
            let mut patch = json!({"mods": rebased});
            // `cover` is derived from the copied file.
            patch["cover"] = json!(s.paths.cover_file(&created.id)?.is_file());
            let p = s
                .update_profile(&created.id, &patch)?
                .cloned()
                .ok_or_else(AppError::profile_not_found)?;
            Ok(crate::types::view_of(&s.paths, &p, &s.config, &bundle))
        })
    }

    // ---- accounts ----

    pub fn account_select(&self, uuid: String) -> Res<ConfigView> {
        self.inner.with_store(|s| {
            if !s.accounts.iter().any(|a| a.uuid == uuid) {
                return Err(AppError::new("account_not_found", "Account not found"));
            }
            Ok(ConfigView::from(
                s.set_config(&json!({"selectedAccount": uuid}))?,
            ))
        })
    }

    // ---- versions ----

    pub async fn versions_manifest(&self, force: bool) -> Res<VersionManifest> {
        let paths = self.inner.paths.clone();
        let raw = get_version_manifest(
            &paths,
            &self.inner.opts.manifest_url,
            force,
            &self.inner.opts.fetch,
        )
        .await?;
        Ok(parse_manifest(&raw, installed_versions(&paths)))
    }

    pub async fn loader_versions(
        &self,
        loader: String,
        mc_version: String,
    ) -> Res<Vec<LoaderVersion>> {
        let l = parse_loader(&loader)?;
        if l == mango_loaders::Loader::Vanilla {
            return Ok(Vec::new());
        }
        let loaders = mango_loaders::Loaders::new(self.inner.opts.loader_endpoints.clone());
        Ok(loaders
            .list_versions(l, &mc_version)
            .await?
            .into_iter()
            .map(|v| LoaderVersion {
                version: v.version,
                stable: v.stable,
            })
            .collect())
    }

    // ---- games ----

    pub fn game_running(&self) -> Vec<String> {
        let mut ids: Vec<String> = lock(&self.inner.games).keys().cloned().collect();
        ids.sort();
        ids
    }

    /// `true` when a stop signal was sent.
    pub fn game_stop(&self, profile_id: String) -> Res<bool> {
        validate_profile_id(&profile_id)?;
        let game = lock(&self.inner.games).get(&profile_id).cloned();
        match game {
            Some(g) => {
                g.stop()?;
                Ok(true)
            }
            None => Ok(false),
        }
    }

    /// Native picker for one image (png/jpg/jpeg/webp/gif); `None` = cancelled.
    /// The path goes to `profile_icon_set_image` / `profile_background_set_image`,
    /// which validate it themselves.
    pub async fn pick_image_file(&self, title: Option<String>) -> Res<Option<String>> {
        let dialog = self.inner.dialog.clone();
        let picked = dialog.pick_image(title).await;
        Ok(self.grant_picked(GrantKind::Image, picked))
    }

    /// Native picker for a file to import (`.mrpack`, Prism/MultiMC `.zip`); `None` = cancelled. The path goes to
    /// `instance_import_plan` / `instance_import`.
    pub async fn pick_import_file(&self, title: Option<String>) -> Res<Option<String>> {
        let dialog = self.inner.dialog.clone();
        let picked = dialog.pick_import(title).await;
        Ok(self.grant_picked(GrantKind::File, picked))
    }

    /// Remember a path a dialog returned (see `path_grants`).
    fn grant_picked(&self, kind: GrantKind, picked: Option<PathBuf>) -> Option<String> {
        let p = picked?;
        self.inner.grants.grant(kind, &p);
        Some(p.display().to_string())
    }

    /// Native folder picker; `None` = cancelled. The caller stores the path
    /// through the command that needs the folder, which validates it.
    pub async fn pick_folder(&self, title: Option<String>) -> Res<Option<String>> {
        let dialog = self.inner.dialog.clone();
        let picked = dialog.pick_folder(title).await;
        Ok(self.grant_picked(GrantKind::Folder, picked))
    }

    /// Native "save as" picker; `None` = cancelled. `default_name` is only a
    /// suggestion (reduced to a bare file name). The caller passes the chosen
    /// path to the command that writes the file.
    pub async fn pick_save_file(
        &self,
        title: Option<String>,
        default_name: String,
    ) -> Res<Option<String>> {
        let name: String = Path::new(&default_name)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .chars()
            .filter(|c| !c.is_control())
            .take(200)
            .collect();
        let name = if name.trim().is_empty() {
            "export".to_string()
        } else {
            name
        };
        let dialog = self.inner.dialog.clone();
        let picked = dialog.pick_save(title, name).await;
        Ok(self.grant_picked(GrantKind::Save, picked))
    }

    /// A user-chosen output file: absolute, not a UNC/device path, not in the
    /// launcher data dir, the bundled resources or `forbidden`, not a
    /// symlink, and (when strict) picked with `pick_save_file`.
    pub(crate) fn checked_out_path(&self, raw: &str, forbidden: &[&Path]) -> Res<PathBuf> {
        crate::path_grants::reject_special_path(raw)?;
        let root = self.inner.paths.root();
        let bundle = self.inner.opts.bundle_dir.as_path();
        let mut all: Vec<&Path> = forbidden.to_vec();
        all.push(root);
        all.push(bundle);
        let out = crate::outfile::validate_out_path(raw, &all)?;
        self.inner.grants.consume(GrantKind::Save, raw)?;
        Ok(out)
    }

    /// A user-chosen file to read: not a UNC/device path and (when strict)
    /// picked with the matching `pick_*` command.
    pub(crate) fn checked_in_path(&self, raw: &str, kind: GrantKind) -> Res<()> {
        crate::path_grants::reject_special_path(raw)?;
        self.inner.grants.consume(kind, raw)
    }

    /// Returns the path; opens the OS file manager when enabled.
    pub fn open_folder(&self, target: String) -> Res<String> {
        let paths = &self.inner.paths;
        let dir = match target.as_str() {
            "root" => paths.root().to_path_buf(),
            "logs" => paths.logs(),
            id => paths.instance_dir(id)?,
        };
        std::fs::create_dir_all(&dir)?;
        self.open_dir(&dir);
        Ok(dir.display().to_string())
    }

    pub(crate) fn open_dir(&self, dir: &Path) {
        if let Err(e) = self.inner.host.opener.open(dir) {
            tracing::warn!("could not open {}: {e}", dir.display());
        }
    }

    pub(crate) fn is_busy(&self, id: &str) -> bool {
        lock(&self.inner.games).contains_key(id) || lock(&self.inner.launching).contains(id)
    }
}

/// Where verified updates are kept until installed. Outside `cache/`, which
/// the storage manager may clear. The shell's installer must be built for
/// exactly this directory.
pub fn updates_dir(paths: &Paths) -> PathBuf {
    paths.root().join("updates")
}

fn build_updater(
    paths: &Paths,
    opts: &ServiceOptions,
    installer: Arc<dyn mango_updater::Installer>,
) -> Updater {
    let version =
        semver::Version::parse(&opts.app_version).unwrap_or_else(|_| semver::Version::new(0, 0, 0));
    let mut cfg = UpdaterConfig::new(
        opts.update_manifest_url.clone(),
        version,
        updates_dir(paths),
    );
    cfg.dev = opts.update_dev;
    cfg.fetch = opts.fetch.clone();
    cfg.public_key_b64 = opts.update_public_key.clone();
    if let Some(p) = &opts.update_platform {
        cfg.platform = p.clone();
    }
    if let Some(p) = &opts.update_policy {
        cfg.policy = p.clone();
    }
    Updater::new(cfg, installer)
}

/// One of vanilla, fabric, quilt, forge, neoforge; nothing else (not even "").
fn parse_loader(raw: &str) -> Res<mango_loaders::Loader> {
    match mango_loaders::Loader::parse(raw) {
        Some(l) if !raw.is_empty() => Ok(l),
        _ => {
            let shown: String = raw.chars().take(40).collect();
            Err(AppError::new(
                "validation",
                format!(
                    "Invalid loader '{shown}' (expected vanilla, fabric, quilt, forge or neoforge)"
                ),
            ))
        }
    }
}

fn parse_manifest(raw: &Value, installed: Vec<String>) -> VersionManifest {
    let s = |v: Option<&Value>| v.and_then(Value::as_str).unwrap_or_default().to_string();
    let latest = raw.get("latest");
    VersionManifest {
        latest: Latest {
            release: s(latest.and_then(|l| l.get("release"))),
            snapshot: s(latest.and_then(|l| l.get("snapshot"))),
        },
        versions: raw
            .get("versions")
            .and_then(Value::as_array)
            .map(|list| {
                list.iter()
                    .filter_map(|v| {
                        let id = v.get("id")?.as_str()?.to_string();
                        Some(VersionEntry {
                            id,
                            kind: s(v.get("type")),
                            release_time: s(v.get("releaseTime")),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default(),
        installed,
    }
}

/// Recursive copy that skips symlinks.
fn copy_dir(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        let dest = to.join(entry.file_name());
        if kind.is_dir() {
            copy_dir(&entry.path(), &dest)?;
        } else if kind.is_file() {
            std::fs::copy(entry.path(), dest)?;
        }
    }
    Ok(())
}

#[cfg(test)]
pub(crate) fn parse_manifest_for_test(raw: &Value) -> VersionManifest {
    parse_manifest(raw, Vec::new())
}
