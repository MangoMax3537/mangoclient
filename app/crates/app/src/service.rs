//! The command surface of `docs/ipc.md`.

use crate::error::{AppError, Res};
use crate::events::{Event, EventSink, LaunchStateEvent, ProgressThrottle};
use crate::host::Host;
use crate::types::{
    installed_versions, AppState, ConfigView, Latest, LaunchState, LoaderVersion, ProfileView,
    PublicAccount, VersionEntry, VersionManifest,
};
use mango_core::{validate_profile_id, Account, Paths, Store};
use mango_minecraft::{get_version_manifest, VERSION_MANIFEST_URL};
use mango_net::FetchOptions;
use mango_runtime::GameInstance;
use mango_servers::{PingOptions, ServerIcons};
use mango_updater::{NoopInstaller, Updater, UpdaterConfig};
use serde_json::{json, Map, Value};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

pub const DEFAULT_UPDATE_URL: &str =
    "https://github.com/mangomax/mangoclient/releases/latest/download/latest.json";

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
    /// Retry/timeout policy for the service's own HTTP calls.
    pub fetch: FetchOptions,
    /// Server list ping settings.
    pub ping: PingOptions,
    /// Partner servers, shown first and starred.
    pub partners: Vec<mango_servers::FeaturedServer>,
    /// `latest.json` of the updater.
    pub update_manifest_url: String,
    /// Unpackaged dev run: the updater reports `disabled`.
    pub update_dev: bool,
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
}

impl Default for ServiceOptions {
    fn default() -> Self {
        let bundle_dir = match std::env::var_os("MANGO_BUNDLE_DIR").filter(|d| !d.is_empty()) {
            Some(dir) => PathBuf::from(dir),
            // Development checkout: crates/app -> app -> repo root.
            None => Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../..")
                .join("src/main/assets"),
        };
        ServiceOptions {
            bundle_dir,
            manifest_url: VERSION_MANIFEST_URL.to_string(),
            loader_endpoints: mango_loaders::Endpoints::default(),
            app_version: env!("CARGO_PKG_VERSION").to_string(),
            open_folders: true,
            modrinth_api: mango_modrinth::API.to_string(),
            mclogs_api: "https://api.mclo.gs/1/log".to_string(),
            fetch: FetchOptions::default(),
            ping: PingOptions::default(),
            partners: mango_servers::featured_servers(),
            update_manifest_url: std::env::var("MANGO_UPDATE_URL")
                .ok()
                .filter(|u| !u.is_empty())
                .unwrap_or_else(|| DEFAULT_UPDATE_URL.to_string()),
            update_dev: cfg!(debug_assertions),
            auto_update_check: true,
            api_base: None,
            watch_interval: Duration::from_secs(2),
            icon_hosts: vec!["cdn.modrinth.com".to_string()],
            icon_allow_http: false,
        }
    }
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
        let modrinth = mango_modrinth::Client::with_options(&opts.modrinth_api, opts.fetch.clone());
        let icons = Arc::new(ServerIcons::in_root(paths.root()));
        let updater = build_updater(&paths, &opts);
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
            }),
        };
        svc.adopt_sessions();
        Ok(svc)
    }

    pub fn paths(&self) -> &Paths {
        &self.inner.paths
    }

    pub fn app_state(&self) -> Res<AppState> {
        let inner = &self.inner;
        let (config, profiles, accounts) = inner.with_store(|s| {
            let now = now_ms() as i64;
            (
                ConfigView::from(&s.config),
                s.profiles.iter().map(ProfileView::from).collect(),
                s.accounts
                    .iter()
                    .map(|a| PublicAccount::from_account(a, now))
                    .collect(),
            )
        });
        Ok(AppState {
            version: inner.opts.app_version.clone(),
            config,
            profiles,
            accounts,
            total_ram_mb: mango_core::model::total_ram_mb(),
            running: self.game_running(),
            root_dir: inner.paths.root().display().to_string(),
        })
    }

    pub fn config_set(&self, patch: Value) -> Res<ConfigView> {
        let view = self
            .inner
            .with_store(|s| s.set_config(&patch).map(ConfigView::from))?;
        // Opting in/out takes effect immediately (needs a runtime to spawn on).
        self.sync_telemetry();
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
        let mut data = json!({"name": name, "mcVersion": mc_version, "loader": loader});
        if let Some(lv) = loader_version.filter(|v| !v.is_empty()) {
            data["loaderVersion"] = Value::String(lv);
        }
        let p = self.inner.with_store(|s| s.add_profile(&data))?;
        Ok(ProfileView::from(&p))
    }

    /// Only the user-editable fields of `ProfileView` are applied; derived
    /// fields (`coverUrl`, `modCount`, `playTimeMs` ...) are ignored.
    pub fn profile_update(&self, id: String, patch: Value) -> Res<ProfileView> {
        validate_profile_id(&id)?;
        let Some(obj) = patch.as_object() else {
            return Err(AppError::new("validation", "Patch must be an object"));
        };
        let mut clean = Map::new();
        for key in [
            "name",
            "mcVersion",
            "loader",
            "color",
            "ram",
            "javaArgs",
            "mangoConfig",
        ] {
            if let Some(v) = obj.get(key) {
                clean.insert(key.to_string(), v.clone());
            }
        }
        if let Some(v) = obj.get("loaderVersion") {
            // The UI uses null where the store uses "".
            let v = if v.is_null() { json!("") } else { v.clone() };
            clean.insert("loaderVersion".into(), v);
        }
        self.inner.with_store(|s| {
            s.update_profile(&id, &Value::Object(clean))?
                .map(ProfileView::from)
                .ok_or_else(AppError::profile_not_found)
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

        self.inner.with_store(|s| {
            let mut patch = json!({"mods": rebased});
            // `cover` is derived from the copied file.
            patch["cover"] = json!(s.paths.cover_file(&created.id)?.is_file());
            s.update_profile(&created.id, &patch)?
                .map(ProfileView::from)
                .ok_or_else(AppError::profile_not_found)
        })
    }

    // ---- accounts ----

    pub fn account_add_offline(&self, name: String) -> Res<PublicAccount> {
        let acc = mango_auth::offline_account(&name)?;
        // Same JSON shape on disk, so a round trip converts between the models.
        let core: Account = serde_json::from_value(serde_json::to_value(&acc)?)?;
        let public = PublicAccount::from_account(&core, now_ms() as i64);
        self.inner.with_store(|s| s.upsert_account(core))?;
        Ok(public)
    }

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

    pub fn account_remove(&self, uuid: String) -> Res<()> {
        self.inner
            .with_store(|s| s.remove_account(&uuid))
            .map_err(Into::into)
    }

    pub async fn account_sign_in_microsoft(&self) -> Res<Option<PublicAccount>> {
        Err(AppError::not_implemented("Microsoft sign-in"))
    }

    // ---- versions ----

    pub async fn versions_manifest(&self, force: bool) -> Res<VersionManifest> {
        let paths = self.inner.paths.clone();
        let raw = get_version_manifest(
            &paths,
            &self.inner.opts.manifest_url,
            force,
            &FetchOptions::default(),
        )
        .await?;
        Ok(parse_manifest(&raw, installed_versions(&paths)))
    }

    pub async fn loader_versions(
        &self,
        loader: String,
        mc_version: String,
    ) -> Res<Vec<LoaderVersion>> {
        let Some(l) = mango_loaders::Loader::parse(&loader) else {
            return Err(AppError::new("validation", "Invalid loader"));
        };
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
        let game = lock(&self.inner.games).get(&profile_id).cloned();
        match game {
            Some(g) => {
                g.stop()?;
                Ok(true)
            }
            None => Ok(false),
        }
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

fn build_updater(paths: &Paths, opts: &ServiceOptions) -> Updater {
    let version =
        semver::Version::parse(&opts.app_version).unwrap_or_else(|_| semver::Version::new(0, 0, 0));
    let mut cfg = UpdaterConfig::new(
        opts.update_manifest_url.clone(),
        version,
        // Outside `cache/`, which the storage manager may clear.
        paths.root().join("updates"),
    );
    cfg.dev = opts.update_dev;
    cfg.fetch = opts.fetch.clone();
    Updater::new(cfg, Arc::new(NoopInstaller))
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
