//! Thin Tauri shell: one command per entry of `docs/ipc.md`, each forwarding to
//! the managed `mango_app::AppService`. No logic lives here.

use std::sync::{Arc, Mutex};

mod platform_updater;

use mango_app::image_protocol::Image;
use mango_app::{
    AppError, AppService, AppState, BlockIconsStatus, BlockedFile, ClipboardImage, ConfigView,
    ContentItem, ContentType, Event, EventSink, FileDialog, Host, InstallPlan, InstallTarget,
    LaunchResult, LoaderVersion, LogEntry, LogText, LogUpload, LoginOutcome, LoginWindow,
    ModpackPlan, PanoramaSet, ProfileView, ProjectDetail, ProjectVersion, Provider, ProviderStatus,
    PublicAccount, Screenshot, ScreenshotAction, SearchParams, SearchResult, ServerEntry,
    ServerStatus, ServiceOptions, StatsSummary, StorageKey, StorageUsage, ToggleResult,
    UninstallResult, UpdateInfo, UpdateStatus, VersionManifest,
};
use mango_core::Paths;
use serde::Deserialize;
use serde_json::Value;
use tauri::ipc::{InvokeBody, Request};
use tauri::{AppHandle, Emitter, Manager, State, WebviewUrl, WebviewWindowBuilder, WindowEvent};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_dialog::DialogExt;

/// Emits the service events (`accounts:changed`, `launch:*`, `install:progress`, `servers:pinged`,
/// `update:status`, `content:changed`, `blocked:changed`, `blocks:status`, `panorama:ready`).
struct TauriSink(AppHandle);

impl EventSink for TauriSink {
    fn emit(&self, event: Event) {
        // A closed window must not take the game pipeline down.
        let _ = self.0.emit(event.name(), event.payload());
    }
}

type Svc<'a> = State<'a, AppService>;
type Res<T> = Result<T, AppError>;

#[tauri::command]
async fn app_state(svc: Svc<'_>) -> Res<AppState> {
    svc.app_state()
}

#[tauri::command]
async fn config_set(svc: Svc<'_>, patch: Value) -> Res<ConfigView> {
    svc.config_set(patch)
}

#[tauri::command]
async fn profile_create(
    svc: Svc<'_>,
    name: String,
    mc_version: String,
    loader: String,
    loader_version: Option<String>,
) -> Res<ProfileView> {
    svc.profile_create(name, mc_version, loader, loader_version)
}

#[tauri::command]
async fn profile_update(svc: Svc<'_>, id: String, patch: Value) -> Res<ProfileView> {
    svc.profile_update(id, patch)
}

#[tauri::command]
async fn profile_delete(svc: Svc<'_>, id: String) -> Res<()> {
    svc.profile_delete(id)
}

#[tauri::command]
async fn profile_select(svc: Svc<'_>, id: String) -> Res<ConfigView> {
    svc.profile_select(id)
}

#[tauri::command]
async fn profile_duplicate(svc: Svc<'_>, id: String) -> Res<ProfileView> {
    svc.profile_duplicate(id).await
}

#[tauri::command]
async fn account_add_offline(svc: Svc<'_>, name: String) -> Res<PublicAccount> {
    svc.account_add_offline(name)
}

#[tauri::command]
async fn account_select(svc: Svc<'_>, uuid: String) -> Res<ConfigView> {
    svc.account_select(uuid)
}

#[tauri::command]
async fn account_remove(svc: Svc<'_>, uuid: String) -> Res<()> {
    svc.account_remove(uuid)
}

#[tauri::command]
async fn account_sign_in_microsoft(svc: Svc<'_>) -> Res<Option<PublicAccount>> {
    svc.account_sign_in_microsoft().await
}

#[tauri::command]
async fn account_refresh(svc: Svc<'_>, uuid: String) -> Res<PublicAccount> {
    svc.account_refresh(uuid).await
}

#[tauri::command]
async fn open_external(svc: Svc<'_>, url: String) -> Res<()> {
    svc.open_external(url)
}

#[tauri::command]
async fn versions_manifest(svc: Svc<'_>, force: Option<bool>) -> Res<VersionManifest> {
    svc.versions_manifest(force.unwrap_or(false)).await
}

#[tauri::command]
async fn loader_versions(
    svc: Svc<'_>,
    loader: String,
    mc_version: String,
) -> Res<Vec<LoaderVersion>> {
    svc.loader_versions(loader, mc_version).await
}

#[tauri::command]
async fn game_launch(
    svc: Svc<'_>,
    profile_id: String,
    quick_join: Option<String>,
) -> Res<LaunchResult> {
    svc.game_launch(profile_id, quick_join).await
}

#[tauri::command]
async fn game_stop(svc: Svc<'_>, profile_id: String) -> Res<bool> {
    svc.game_stop(profile_id)
}

#[tauri::command]
async fn game_running(svc: Svc<'_>) -> Res<Vec<String>> {
    Ok(svc.game_running())
}

#[tauri::command]
async fn open_folder(svc: Svc<'_>, target: String) -> Res<String> {
    svc.open_folder(target)
}

// ---- content and Modrinth ----

#[tauri::command]
async fn content_list(svc: Svc<'_>, profile_id: String) -> Res<Vec<ContentItem>> {
    svc.content_list(profile_id).await
}

#[tauri::command]
async fn content_toggle(
    svc: Svc<'_>,
    profile_id: String,
    project_id: String,
    enabled: bool,
) -> Res<ToggleResult> {
    svc.content_toggle(profile_id, project_id, enabled).await
}

#[tauri::command]
async fn content_uninstall(
    svc: Svc<'_>,
    profile_id: String,
    project_id: String,
) -> Res<UninstallResult> {
    svc.content_uninstall(profile_id, project_id).await
}

#[tauri::command]
async fn content_check_updates(svc: Svc<'_>, profile_id: String) -> Res<Vec<UpdateInfo>> {
    svc.content_check_updates(profile_id).await
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ContentFolderArgs {
    profile_id: String,
    #[serde(rename = "type")]
    kind: ContentType,
}

/// The contract names this argument `type`, a keyword: read the raw JSON
/// body instead of a typed parameter.
#[tauri::command]
async fn content_open_folder(svc: Svc<'_>, request: Request<'_>) -> Res<String> {
    let InvokeBody::Json(body) = request.body() else {
        return Err(AppError::new("validation", "Expected a JSON body"));
    };
    let args: ContentFolderArgs = serde_json::from_value(body.clone())
        .map_err(|e| AppError::new("validation", e.to_string()))?;
    svc.content_open_folder(args.profile_id, args.kind)
}

#[tauri::command]
async fn modrinth_search(svc: Svc<'_>, params: SearchParams) -> Res<SearchResult> {
    svc.modrinth_search(params).await
}

#[tauri::command]
async fn modrinth_project(svc: Svc<'_>, project_id: String) -> Res<ProjectDetail> {
    svc.modrinth_project(project_id).await
}

#[tauri::command]
async fn modrinth_versions(
    svc: Svc<'_>,
    project_id: String,
    profile_id: Option<String>,
) -> Res<Vec<ProjectVersion>> {
    svc.modrinth_versions(project_id, profile_id).await
}

#[tauri::command]
async fn modrinth_install(
    svc: Svc<'_>,
    profile_id: String,
    project_id: String,
    version_id: Option<String>,
) -> Res<Vec<ContentItem>> {
    svc.modrinth_install(profile_id, project_id, version_id)
        .await
}

#[tauri::command]
async fn modrinth_install_modpack(
    svc: Svc<'_>,
    project_id: String,
    version_id: Option<String>,
    name: Option<String>,
) -> Res<ProfileView> {
    svc.modrinth_install_modpack(project_id, version_id, name)
        .await
}

// ---- providers (Modrinth + CurseForge), install planning, modpacks ----

#[tauri::command]
async fn providers_status(svc: Svc<'_>) -> Res<Vec<ProviderStatus>> {
    svc.providers_status()
}

#[tauri::command]
async fn search_projects(svc: Svc<'_>, params: SearchParams) -> Res<SearchResult> {
    svc.search_projects(params).await
}

#[tauri::command]
async fn project_detail(
    svc: Svc<'_>,
    provider: Provider,
    project_id: String,
) -> Res<ProjectDetail> {
    svc.project_detail(provider, project_id).await
}

#[tauri::command]
async fn project_versions(
    svc: Svc<'_>,
    provider: Provider,
    project_id: String,
    profile_id: Option<String>,
) -> Res<Vec<ProjectVersion>> {
    svc.project_versions(provider, project_id, profile_id).await
}

#[tauri::command]
async fn install_targets(
    svc: Svc<'_>,
    provider: Provider,
    project_id: String,
    version_id: Option<String>,
) -> Res<Vec<InstallTarget>> {
    svc.install_targets(provider, project_id, version_id).await
}

#[tauri::command]
async fn install_plan(
    svc: Svc<'_>,
    provider: Provider,
    project_id: String,
    version_id: Option<String>,
    profile_id: String,
) -> Res<InstallPlan> {
    svc.install_plan(provider, project_id, version_id, profile_id)
        .await
}

#[tauri::command]
async fn install_apply(
    svc: Svc<'_>,
    provider: Provider,
    project_id: String,
    version_id: Option<String>,
    profile_id: String,
    include: Vec<String>,
) -> Res<Vec<ContentItem>> {
    svc.install_apply(provider, project_id, version_id, profile_id, include)
        .await
}

#[tauri::command]
async fn modpack_plan(
    svc: Svc<'_>,
    provider: Provider,
    project_id: String,
    version_id: Option<String>,
) -> Res<ModpackPlan> {
    svc.modpack_plan(provider, project_id, version_id).await
}

#[tauri::command]
async fn modpack_install(
    svc: Svc<'_>,
    provider: Provider,
    project_id: String,
    version_id: Option<String>,
    name: Option<String>,
) -> Res<ProfileView> {
    svc.modpack_install(provider, project_id, version_id, name)
        .await
}

// ---- blocked CurseForge files (Part 5) ----

#[tauri::command]
fn blocked_files(svc: Svc<'_>, profile_id: String) -> Res<Vec<BlockedFile>> {
    svc.blocked_files(profile_id)
}

#[tauri::command]
async fn blocked_watch_start(svc: Svc<'_>, profile_id: String) -> Res<Vec<String>> {
    svc.blocked_watch_start(profile_id).await
}

#[tauri::command]
fn blocked_watch_stop(svc: Svc<'_>, profile_id: String) -> Res<()> {
    svc.blocked_watch_stop(profile_id)
}

#[tauri::command]
fn blocked_skip(svc: Svc<'_>, profile_id: String, file_name: String) -> Res<Vec<BlockedFile>> {
    svc.blocked_skip(profile_id, file_name)
}

#[tauri::command]
fn blocked_open_folder(svc: Svc<'_>) -> Res<String> {
    svc.blocked_open_folder()
}

// ---- instance icons and backgrounds (Parts 6 and 7) ----

#[tauri::command]
fn block_icons_status(svc: Svc<'_>) -> Res<BlockIconsStatus> {
    Ok(svc.block_icons_status())
}

#[tauri::command]
async fn block_icons_prepare(svc: Svc<'_>, force: Option<bool>) -> Res<BlockIconsStatus> {
    svc.block_icons_prepare(force.unwrap_or(false)).await
}

#[tauri::command]
fn profile_icon_set_block(svc: Svc<'_>, id: String, block: String) -> Res<ProfileView> {
    svc.profile_icon_set_block(id, block)
}

#[tauri::command]
async fn profile_icon_set_image(svc: Svc<'_>, id: String, file_path: String) -> Res<ProfileView> {
    svc.profile_icon_set_image(id, file_path).await
}

#[tauri::command]
fn profile_icon_clear(svc: Svc<'_>, id: String) -> Res<ProfileView> {
    svc.profile_icon_clear(id)
}

#[tauri::command]
async fn panorama_for_profile(svc: Svc<'_>, id: String) -> Res<Option<PanoramaSet>> {
    svc.panorama_for_profile(id).await
}

#[tauri::command]
async fn profile_background_set_screenshot(
    svc: Svc<'_>,
    id: String,
    name: String,
) -> Res<ProfileView> {
    svc.profile_background_set_screenshot(id, name).await
}

#[tauri::command]
async fn profile_background_set_image(
    svc: Svc<'_>,
    id: String,
    file_path: String,
) -> Res<ProfileView> {
    svc.profile_background_set_image(id, file_path).await
}

#[tauri::command]
fn profile_background_set_style(
    svc: Svc<'_>,
    id: String,
    dim: Option<f64>,
    blur: Option<f64>,
) -> Res<ProfileView> {
    svc.profile_background_set_style(id, dim, blur)
}

#[tauri::command]
fn profile_background_set_kind(
    svc: Svc<'_>,
    id: String,
    kind: String,
    wallpaper: Option<String>,
) -> Res<ProfileView> {
    svc.profile_background_set_kind(id, kind, wallpaper)
}

#[tauri::command]
async fn profile_icon_use_modpack_logo(svc: Svc<'_>, id: String) -> Res<ProfileView> {
    svc.profile_icon_use_modpack_logo(id).await
}

#[tauri::command]
fn profile_background_clear(svc: Svc<'_>, id: String, none: Option<bool>) -> Res<ProfileView> {
    svc.profile_background_clear(id, none.unwrap_or(false))
}

#[tauri::command]
async fn pick_image_file(svc: Svc<'_>, title: Option<String>) -> Res<Option<String>> {
    svc.pick_image_file(title).await
}

#[tauri::command]
async fn pick_folder(svc: Svc<'_>, title: Option<String>) -> Res<Option<String>> {
    svc.pick_folder(title).await
}

// ---- servers, media, logs, stats, storage ----

#[tauri::command]
async fn servers_list(svc: Svc<'_>) -> Res<Vec<ServerEntry>> {
    svc.servers_list().await
}

#[tauri::command]
async fn servers_ping_all(svc: Svc<'_>) -> Res<()> {
    svc.servers_ping_all().await
}

#[tauri::command]
async fn servers_ping(svc: Svc<'_>, address: String) -> Res<ServerStatus> {
    svc.servers_ping(address).await
}

#[tauri::command]
async fn screenshots_list(svc: Svc<'_>, profile_id: String) -> Res<Vec<Screenshot>> {
    svc.screenshots_list(profile_id).await
}

#[tauri::command]
async fn screenshot_action(
    svc: Svc<'_>,
    profile_id: String,
    name: String,
    action: ScreenshotAction,
) -> Res<()> {
    svc.screenshot_action(profile_id, name, action).await
}

#[tauri::command]
async fn logs_list(svc: Svc<'_>, profile_id: String) -> Res<Vec<LogEntry>> {
    svc.logs_list(profile_id).await
}

#[tauri::command]
async fn logs_read(svc: Svc<'_>, profile_id: String, id: String) -> Res<LogText> {
    svc.logs_read(profile_id, id).await
}

#[tauri::command]
async fn logs_upload(svc: Svc<'_>, profile_id: String, id: String) -> Res<LogUpload> {
    svc.logs_upload(profile_id, id).await
}

#[tauri::command]
async fn logs_delete(svc: Svc<'_>, profile_id: String, id: String) -> Res<()> {
    svc.logs_delete(profile_id, id).await
}

#[tauri::command]
async fn stats_summary(svc: Svc<'_>, days: u32) -> Res<StatsSummary> {
    svc.stats_summary(days)
}

#[tauri::command]
async fn storage_usage(svc: Svc<'_>) -> Res<Vec<StorageUsage>> {
    svc.storage_usage().await
}

#[tauri::command]
async fn storage_clear(svc: Svc<'_>, key: StorageKey) -> Res<Vec<StorageUsage>> {
    svc.storage_clear(key).await
}

// ---- updates ----

#[tauri::command]
async fn update_status(svc: Svc<'_>) -> Res<UpdateStatus> {
    Ok(svc.update_status())
}

#[tauri::command]
async fn update_check(svc: Svc<'_>) -> Res<UpdateStatus> {
    svc.update_check().await
}

#[tauri::command]
async fn update_install(svc: Svc<'_>) -> Res<bool> {
    svc.update_install()
}

// ---- clipboard and the mangoimg:// protocol ----

/// Screenshot copy: the picture itself goes on the system clipboard.
/// `Image::from_path` needs tauri's `image-png` feature, so only PNGs copy;
/// a JPEG fails with the `clipboard` error code.
struct TauriClipboard(AppHandle);

impl ClipboardImage for TauriClipboard {
    fn copy_image(&self, file: &std::path::Path) -> Result<(), String> {
        let image = tauri::image::Image::from_path(file).map_err(|e| e.to_string())?;
        self.0
            .clipboard()
            .write_image(&image)
            .map_err(|e| e.to_string())
    }
}

/// Native pickers through the dialog plugin. The blocking dialog runs on a
/// blocking thread so the async runtime stays free.
struct TauriDialog(AppHandle);

impl TauriDialog {
    async fn pick(&self, title: Option<String>, folder: bool) -> Option<std::path::PathBuf> {
        let app = self.0.clone();
        tauri::async_runtime::spawn_blocking(move || {
            let mut dialog = app.dialog().file();
            if let Some(t) = title {
                dialog = dialog.set_title(t);
            }
            let picked = if folder {
                dialog.blocking_pick_folder()
            } else {
                dialog
                    .add_filter("Images", &["png", "jpg", "jpeg", "webp", "gif"])
                    .blocking_pick_file()
            };
            picked.and_then(|p| p.into_path().ok())
        })
        .await
        .ok()
        .flatten()
    }
}

#[async_trait::async_trait]
impl FileDialog for TauriDialog {
    async fn pick_image(&self, title: Option<String>) -> Option<std::path::PathBuf> {
        self.pick(title, false).await
    }

    async fn pick_folder(&self, title: Option<String>) -> Option<std::path::PathBuf> {
        self.pick(title, true).await
    }
}

// ---- Microsoft sign-in window ----

const LOGIN_LABEL: &str = "ms-login";

type Outcome = Arc<Mutex<Option<tokio::sync::oneshot::Sender<LoginOutcome>>>>;

/// First outcome wins; later ones (window destroyed after a redirect, ...)
/// are dropped.
fn resolve(tx: &Outcome, outcome: LoginOutcome) {
    let sender = tx.lock().unwrap_or_else(|e| e.into_inner()).take();
    if let Some(sender) = sender {
        let _ = sender.send(outcome);
    }
}

/// Isolated sign-in window: non-persistent data store, no devtools, no IPC
/// (the window label is not listed in any capability), navigation limited to
/// the Microsoft hosts by `mango_app::login_navigation_allowed`.
struct TauriLogin(AppHandle);

#[async_trait::async_trait]
impl LoginWindow for TauriLogin {
    async fn authorize(&self, authorize_url: &str, redirect_prefix: &str) -> LoginOutcome {
        let Ok(url) = authorize_url.parse::<tauri::Url>() else {
            return LoginOutcome::Error("invalid sign-in URL".into());
        };
        if !mango_app::login_navigation_allowed(authorize_url) {
            return LoginOutcome::Error("sign-in URL is not allowed".into());
        }
        // A stale window from an earlier attempt must not linger.
        if let Some(old) = self.0.get_webview_window(LOGIN_LABEL) {
            let _ = old.destroy();
        }
        let (tx, rx) = tokio::sync::oneshot::channel::<LoginOutcome>();
        let tx: Outcome = Arc::new(Mutex::new(Some(tx)));

        let nav_tx = tx.clone();
        let prefix = redirect_prefix.to_string();
        let built = WebviewWindowBuilder::new(&self.0, LOGIN_LABEL, WebviewUrl::External(url))
            .title("Microsoft sign-in")
            .inner_size(480.0, 680.0)
            .center()
            .incognito(true)
            .devtools(false)
            .on_navigation(move |target| {
                let target = target.as_str();
                if mango_app::is_login_redirect(target, &prefix) {
                    // Hand the redirect to the service and never load it.
                    resolve(&nav_tx, LoginOutcome::Code(target.to_string()));
                    return false;
                }
                mango_app::login_navigation_allowed(target)
            })
            .build();
        let window = match built {
            Ok(w) => w,
            Err(e) => return LoginOutcome::Error(e.to_string()),
        };
        let close_tx = tx.clone();
        window.on_window_event(move |event| {
            if matches!(event, WindowEvent::Destroyed) {
                resolve(&close_tx, LoginOutcome::Cancelled);
            }
        });

        let outcome = rx.await.unwrap_or(LoginOutcome::Cancelled);
        // Closes the window after a redirect; a no-op when the user did.
        if let Some(w) = self.0.get_webview_window(LOGIN_LABEL) {
            let _ = w.destroy();
        }
        outcome
    }
}

/// Opens help links with the OS default browser. Policy is enforced by
/// `AppService::open_external`.
struct TauriExternal;

impl mango_app::ExternalOpener for TauriExternal {
    fn open_url(&self, url: &str) -> Result<(), String> {
        <mango_app::SystemExternal as mango_app::ExternalOpener>::open_url(
            &mango_app::SystemExternal { enabled: true },
            url,
        )
    }
}

fn http_response(status: u16, mime: &str, body: Vec<u8>) -> tauri::http::Response<Vec<u8>> {
    tauri::http::Response::builder()
        .status(status)
        .header("Content-Type", mime)
        .header("X-Content-Type-Options", "nosniff")
        .header("Access-Control-Allow-Origin", "*")
        // Heads and covers change in place; the webview must revalidate.
        .header("Cache-Control", "no-cache")
        .body(body)
        .unwrap_or_else(|_| tauri::http::Response::new(Vec::new()))
}

/// All policy lives in `mango_app::image_protocol`; this only moves bytes.
async fn image_response(svc: &AppService, url: &str) -> tauri::http::Response<Vec<u8>> {
    match svc.image(url).await {
        Ok(Image::Bytes { data, mime }) => http_response(200, mime, data),
        Ok(Image::File { path, mime }) => {
            match tauri::async_runtime::spawn_blocking(move || std::fs::read(path)).await {
                Ok(Ok(data)) => http_response(200, mime, data),
                _ => http_response(404, "text/plain", Vec::new()),
            }
        }
        Err(e) => http_response(e.status(), "text/plain", Vec::new()),
    }
}

pub fn run() {
    let result = tauri::Builder::default()
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_dialog::init())
        .register_asynchronous_uri_scheme_protocol("mangoimg", |ctx, request, responder| {
            let svc = ctx.app_handle().try_state::<AppService>();
            let url = request.uri().to_string();
            match svc {
                Some(svc) => {
                    let svc: AppService = svc.inner().clone();
                    tauri::async_runtime::spawn(async move {
                        responder.respond(image_response(&svc, &url).await);
                    });
                }
                None => responder.respond(http_response(503, "text/plain", Vec::new())),
            }
        })
        .setup(|app| {
            let mut opts = ServiceOptions::default();
            // Packaged builds ship the MangoConfig jars as the `mangoconfig`
            // resource; MANGO_BUNDLE_DIR still wins, the dev checkout is the
            // fallback.
            if let Ok(dir) = app.path().resource_dir() {
                opts = opts.with_resource_dir(&dir);
            }
            let sink = Arc::new(TauriSink(app.handle().clone()));
            let host = Host::system(true)
                .with_clipboard(Arc::new(TauriClipboard(app.handle().clone())))
                .with_login(Arc::new(TauriLogin(app.handle().clone())))
                .with_external(Arc::new(TauriExternal))
                .with_dialog(Arc::new(TauriDialog(app.handle().clone())));
            let svc = AppService::with_host(Paths::from_env(), sink, opts, host)?;
            app.manage(svc.clone());
            // Telemetry, update checks and the mods/ watcher need a tokio
            // runtime, which `setup` itself does not run on.
            tauri::async_runtime::spawn(async move { svc.start_background() });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            app_state,
            config_set,
            profile_create,
            profile_update,
            profile_delete,
            profile_select,
            profile_duplicate,
            account_add_offline,
            account_select,
            account_remove,
            account_sign_in_microsoft,
            account_refresh,
            open_external,
            versions_manifest,
            loader_versions,
            game_launch,
            game_stop,
            game_running,
            open_folder,
            content_list,
            content_toggle,
            content_uninstall,
            content_check_updates,
            content_open_folder,
            modrinth_search,
            modrinth_project,
            modrinth_versions,
            modrinth_install,
            modrinth_install_modpack,
            providers_status,
            search_projects,
            project_detail,
            project_versions,
            install_targets,
            install_plan,
            install_apply,
            modpack_plan,
            modpack_install,
            blocked_files,
            blocked_watch_start,
            blocked_watch_stop,
            blocked_skip,
            blocked_open_folder,
            block_icons_status,
            block_icons_prepare,
            profile_icon_set_block,
            profile_icon_set_image,
            profile_icon_clear,
            panorama_for_profile,
            profile_background_set_screenshot,
            profile_background_set_image,
            profile_background_set_style,
            profile_background_set_kind,
            profile_icon_use_modpack_logo,
            profile_background_clear,
            pick_image_file,
            pick_folder,
            servers_list,
            servers_ping_all,
            servers_ping,
            screenshots_list,
            screenshot_action,
            logs_list,
            logs_read,
            logs_upload,
            logs_delete,
            stats_summary,
            storage_usage,
            storage_clear,
            update_status,
            update_check,
            update_install,
        ])
        .run(tauri::generate_context!());
    if let Err(e) = result {
        eprintln!("MangoClient failed to run: {e}");
        std::process::exit(1);
    }
}
