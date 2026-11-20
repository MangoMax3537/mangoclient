//! Thin Tauri shell: one command per entry of `docs/ipc.md`, each forwarding to
//! the managed `mango_app::AppService`. No logic lives here.

use std::sync::Arc;

use mango_app::image_protocol::Image;
use mango_app::{
    AppError, AppService, AppState, ClipboardImage, ConfigView, ContentItem, ContentType, Event,
    EventSink, Host, LaunchResult, LoaderVersion, LogEntry, LogText, LogUpload, ProfileView,
    ProjectDetail, ProjectVersion, PublicAccount, Screenshot, ScreenshotAction, SearchParams,
    SearchResult, ServerEntry, ServerStatus, ServiceOptions, StatsSummary, StorageKey,
    StorageUsage, ToggleResult, UninstallResult, UpdateInfo, UpdateStatus, VersionManifest,
};
use mango_core::Paths;
use serde::Deserialize;
use serde_json::Value;
use tauri::ipc::{InvokeBody, Request};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_clipboard_manager::ClipboardExt;

/// Emits the service events (`launch:*`, `install:progress`, `servers:pinged`,
/// `update:status`, `content:changed`).
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
            // Packaged builds ship the MangoConfig jars as resources/assets.
            if let Ok(dir) = app.path().resource_dir() {
                let bundled = dir.join("assets");
                if bundled.is_dir() {
                    opts.bundle_dir = bundled;
                }
            }
            let sink = Arc::new(TauriSink(app.handle().clone()));
            let host =
                Host::system(true).with_clipboard(Arc::new(TauriClipboard(app.handle().clone())));
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
