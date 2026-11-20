//! Thin Tauri shell: one command per entry of `docs/ipc.md`, each forwarding to
//! the managed `mango_app::AppService`. No logic lives here.

use std::sync::Arc;

use mango_app::{
    AppError, AppService, AppState, ConfigView, Event, EventSink, LaunchResult, LoaderVersion,
    ProfileView, PublicAccount, ServiceOptions, VersionManifest,
};
use mango_core::Paths;
use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager, State};

/// Emits the service events (`launch:state`, `launch:progress`, `launch:log`).
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

pub fn run() {
    let result = tauri::Builder::default()
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
            let svc = AppService::new(Paths::from_env(), sink, opts)?;
            app.manage(svc);
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
        ])
        .run(tauri::generate_context!());
    if let Err(e) = result {
        eprintln!("MangoClient failed to run: {e}");
        std::process::exit(1);
    }
}
