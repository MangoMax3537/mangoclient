//! Name -> function table for every command of `docs/ipc.md`.
//!
//! `src-tauri` exposes the same commands through `#[tauri::command]`; this
//! table lets anything without Tauri (the `mango-dev` HTTP server, tests, a
//! CLI) call the exact same service methods with the exact same camelCase JSON
//! arguments. The unit test `command_table_matches_the_tauri_handlers` keeps
//! this table and `src-tauri/src/lib.rs` from drifting, and the TypeScript
//! `CommandMap` is generated from the same table (see `bindings`).

use crate::error::AppError;
use crate::models::{ContentType, Provider, ScreenshotAction, SearchParams, StorageKey};
use crate::service::AppService;
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::Value;
use ts_rs::TS;

macro_rules! args {
    ($($(#[$sm:meta])* $name:ident { $($(#[$fm:meta])* $f:ident : $t:ty),* $(,)? })*) => {$(
        #[derive(Debug, Clone, Deserialize, TS)]
        #[serde(rename_all = "camelCase", deny_unknown_fields)]
        $(#[$sm])*
        pub struct $name { $( $(#[$fm])* pub $f: $t ),* }
    )*};
}

/// Commands without arguments (`null`, `{}` or no body).
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(type = "Record<string, never>")]
pub struct NoArgs {}

args! {
    ConfigSetArgs {
        #[ts(as = "crate::wire::ConfigPatch")]
        patch: Value,
    }
    ProfileCreateArgs {
        name: String,
        mc_version: String,
        #[ts(as = "mango_core::Loader")]
        loader: String,
        #[ts(optional = nullable)]
        loader_version: Option<String>,
    }
    ProfileUpdateArgs {
        id: String,
        #[ts(as = "crate::wire::ProfilePatch")]
        patch: Value,
    }
    /// A profile (instance) id.
    IdArgs { id: String }
    ProfileIdArgs { profile_id: String }
    NameArgs { name: String }
    UuidArgs { uuid: String }
    UrlArgs { url: String }
    VersionsManifestArgs {
        #[ts(optional = nullable)]
        force: Option<bool>,
    }
    LoaderVersionsArgs {
        #[ts(as = "mango_core::Loader")]
        loader: String,
        mc_version: String,
    }
    GameLaunchArgs {
        profile_id: String,
        #[ts(optional = nullable)]
        quick_join: Option<String>,
    }
    /// `target` is `root`, `logs` or a profile id.
    OpenFolderArgs { target: String }
    ContentToggleArgs { profile_id: String, project_id: String, enabled: bool }
    ProfileProjectArgs { profile_id: String, project_id: String }
    ContentOpenFolderArgs {
        profile_id: String,
        #[serde(rename = "type")]
        kind: ContentType,
    }
    SearchArgs { params: SearchParams }
    ProjectIdArgs { project_id: String }
    ModrinthVersionsArgs {
        project_id: String,
        #[ts(optional = nullable)]
        profile_id: Option<String>,
    }
    ModrinthInstallArgs {
        profile_id: String,
        project_id: String,
        #[ts(optional = nullable)]
        version_id: Option<String>,
    }
    ModrinthInstallModpackArgs {
        project_id: String,
        #[ts(optional = nullable)]
        version_id: Option<String>,
        #[ts(optional = nullable)]
        name: Option<String>,
    }
    ProjectDetailArgs { provider: Provider, project_id: String }
    ProjectVersionsArgs {
        provider: Provider,
        project_id: String,
        #[ts(optional = nullable)]
        profile_id: Option<String>,
    }
    /// Also the arguments of `modpack_plan`.
    InstallTargetsArgs {
        provider: Provider,
        project_id: String,
        #[ts(optional = nullable)]
        version_id: Option<String>,
    }
    InstallPlanArgs {
        provider: Provider,
        project_id: String,
        #[ts(optional = nullable)]
        version_id: Option<String>,
        profile_id: String,
    }
    InstallApplyArgs {
        provider: Provider,
        project_id: String,
        #[ts(optional = nullable)]
        version_id: Option<String>,
        profile_id: String,
        /// Project ids of the optional dependencies to install as well.
        include: Vec<String>,
    }
    ModpackInstallArgs {
        provider: Provider,
        project_id: String,
        #[ts(optional = nullable)]
        version_id: Option<String>,
        #[ts(optional = nullable)]
        name: Option<String>,
    }
    BlockedSkipArgs { profile_id: String, file_name: String }
    BlockIconsPrepareArgs {
        #[ts(optional = nullable)]
        force: Option<bool>,
    }
    ProfileIconSetBlockArgs { id: String, block: String }
    /// Also the arguments of `profile_background_set_image`.
    ProfileImageFileArgs { id: String, file_path: String }
    ProfileBackgroundScreenshotArgs { id: String, name: String }
    ProfileBackgroundStyleArgs {
        id: String,
        /// 0..1; an absent field is left unchanged.
        #[ts(optional = nullable)]
        dim: Option<f64>,
        /// 0..24 px.
        #[ts(optional = nullable)]
        blur: Option<f64>,
    }
    ProfileBackgroundKindArgs {
        id: String,
        #[ts(type = "'auto' | 'none' | 'panorama' | 'wallpaper'")]
        kind: String,
        /// Bundled wallpaper file stem, with `kind: 'wallpaper'`.
        #[ts(optional = nullable)]
        wallpaper: Option<String>,
    }
    ProfileBackgroundClearArgs {
        id: String,
        #[ts(optional = nullable)]
        none: Option<bool>,
    }
    PickArgs {
        #[ts(optional = nullable)]
        title: Option<String>,
    }
    PickSaveArgs {
        #[ts(optional = nullable)]
        title: Option<String>,
        default_name: String,
    }
    WorldBackupArgs {
        profile_id: String,
        world_id: String,
        #[ts(optional = nullable)]
        label: Option<String>,
    }
    WorldDeleteArgs { profile_id: String, world_id: String, make_backup: bool }
    BackupArgs { profile_id: String, backup_id: String }
    BackupRestoreArgs {
        profile_id: String,
        backup_id: String,
        #[ts(optional = nullable)]
        as_new: Option<bool>,
    }
    SnapshotCreateArgs {
        profile_id: String,
        #[ts(optional = nullable)]
        label: Option<String>,
    }
    InstanceExportArgs {
        profile_id: String,
        format: crate::export::ExportFormat,
        include_config: bool,
        out_path: String,
    }
    InstanceImportPlanArgs { file_path: String }
    InstanceImportArgs {
        file_path: String,
        #[ts(optional = nullable)]
        name: Option<String>,
    }
    CrashAnalyzeArgs {
        profile_id: String,
        #[ts(optional = nullable)]
        log_id: Option<String>,
    }
    DiagnosticsExportArgs { profile_id: String, out_path: String }
    AddressArgs { address: String }
    ScreenshotActionArgs { profile_id: String, name: String, action: ScreenshotAction }
    LogFileArgs { profile_id: String, id: String }
    StatsSummaryArgs { days: u32 }
    StorageClearArgs { key: StorageKey }
}

fn parse<T: DeserializeOwned>(command: &str, args: Value) -> Result<T, AppError> {
    // No arguments at all is the empty object.
    let args = if args.is_null() {
        Value::Object(serde_json::Map::new())
    } else {
        args
    };
    serde_json::from_value(args).map_err(|e| AppError::new("bad_args", format!("{command}: {e}")))
}

fn to_json<T: serde::Serialize>(r: Result<T, AppError>) -> Result<Value, AppError> {
    serde_json::to_value(r?).map_err(AppError::from)
}

macro_rules! table {
    ($($name:literal : $args:ty => $res:ty = |$s:ident, $a:ident| $body:expr;)*) => {
        /// Every command, in `docs/ipc.md` order.
        pub const COMMANDS: &[&str] = &[$($name),*];

        /// Call a command by name with camelCase JSON arguments.
        ///
        /// Errors: `unknown_command`, `bad_args` (with the field that failed),
        /// or whatever the service returns.
        pub async fn invoke(
            service: &AppService,
            command: &str,
            args: Value,
        ) -> Result<Value, AppError> {
            match command {
                $($name => {
                    let $a: $args = parse(command, args)?;
                    let $s = service;
                    let result: Result<$res, AppError> = $body;
                    to_json(result)
                })*
                _ => Err(AppError::new(
                    "unknown_command",
                    format!("Unknown command `{command}`"),
                )),
            }
        }

        /// `(command, args type, result type)` as TypeScript source names.
        pub fn command_signatures(cfg: &ts_rs::Config) -> Vec<(&'static str, String, String)> {
            vec![$(($name, <$args as TS>::name(cfg), <$res as TS>::name(cfg))),*]
        }
    };
}

use crate::*;

table! {
    "app_state": NoArgs => AppState = |s, _a| s.app_state();
    "config_set": ConfigSetArgs => ConfigView = |s, a| s.config_set(a.patch);
    "profile_create": ProfileCreateArgs => ProfileView =
        |s, a| s.profile_create(a.name, a.mc_version, a.loader, a.loader_version);
    "profile_update": ProfileUpdateArgs => ProfileView = |s, a| s.profile_update(a.id, a.patch);
    "profile_delete": IdArgs => () = |s, a| s.profile_delete(a.id);
    "profile_select": IdArgs => ConfigView = |s, a| s.profile_select(a.id);
    "profile_duplicate": IdArgs => ProfileView = |s, a| s.profile_duplicate(a.id).await;
    "account_add_offline": NameArgs => PublicAccount = |s, a| s.account_add_offline(a.name);
    "account_select": UuidArgs => ConfigView = |s, a| s.account_select(a.uuid);
    "account_remove": UuidArgs => () = |s, a| s.account_remove(a.uuid);
    "account_sign_in_microsoft": NoArgs => Option<PublicAccount> =
        |s, _a| s.account_sign_in_microsoft().await;
    "account_refresh": UuidArgs => PublicAccount = |s, a| s.account_refresh(a.uuid).await;
    "open_external": UrlArgs => () = |s, a| s.open_external(a.url);
    "versions_manifest": VersionsManifestArgs => VersionManifest =
        |s, a| s.versions_manifest(a.force.unwrap_or(false)).await;
    "loader_versions": LoaderVersionsArgs => Vec<LoaderVersion> =
        |s, a| s.loader_versions(a.loader, a.mc_version).await;
    "game_launch": GameLaunchArgs => LaunchResult =
        |s, a| s.game_launch(a.profile_id, a.quick_join).await;
    "game_stop": ProfileIdArgs => bool = |s, a| s.game_stop(a.profile_id);
    "game_running": NoArgs => Vec<String> = |s, _a| Ok(s.game_running());
    "open_folder": OpenFolderArgs => String = |s, a| s.open_folder(a.target);
    "content_list": ProfileIdArgs => Vec<ContentItem> = |s, a| s.content_list(a.profile_id).await;
    "content_toggle": ContentToggleArgs => ToggleResult =
        |s, a| s.content_toggle(a.profile_id, a.project_id, a.enabled).await;
    "content_uninstall": ProfileProjectArgs => UninstallResult =
        |s, a| s.content_uninstall(a.profile_id, a.project_id).await;
    "content_check_updates": ProfileIdArgs => Vec<UpdateInfo> =
        |s, a| s.content_check_updates(a.profile_id).await;
    "content_open_folder": ContentOpenFolderArgs => String =
        |s, a| s.content_open_folder(a.profile_id, a.kind);
    "modrinth_search": SearchArgs => SearchResult = |s, a| s.modrinth_search(a.params).await;
    "modrinth_project": ProjectIdArgs => ProjectDetail =
        |s, a| s.modrinth_project(a.project_id).await;
    "modrinth_versions": ModrinthVersionsArgs => Vec<ProjectVersion> =
        |s, a| s.modrinth_versions(a.project_id, a.profile_id).await;
    "modrinth_install": ModrinthInstallArgs => Vec<ContentItem> =
        |s, a| s.modrinth_install(a.profile_id, a.project_id, a.version_id).await;
    "modrinth_install_modpack": ModrinthInstallModpackArgs => ProfileView =
        |s, a| s.modrinth_install_modpack(a.project_id, a.version_id, a.name).await;
    "providers_status": NoArgs => Vec<ProviderStatus> = |s, _a| s.providers_status();
    "search_projects": SearchArgs => SearchResult = |s, a| s.search_projects(a.params).await;
    "project_detail": ProjectDetailArgs => ProjectDetail =
        |s, a| s.project_detail(a.provider, a.project_id).await;
    "project_versions": ProjectVersionsArgs => Vec<ProjectVersion> =
        |s, a| s.project_versions(a.provider, a.project_id, a.profile_id).await;
    "install_targets": InstallTargetsArgs => Vec<InstallTarget> =
        |s, a| s.install_targets(a.provider, a.project_id, a.version_id).await;
    "install_plan": InstallPlanArgs => InstallPlan =
        |s, a| s.install_plan(a.provider, a.project_id, a.version_id, a.profile_id).await;
    "install_apply": InstallApplyArgs => Vec<ContentItem> = |s, a| {
        s.install_apply(a.provider, a.project_id, a.version_id, a.profile_id, a.include)
            .await
    };
    "modpack_plan": InstallTargetsArgs => ModpackPlan =
        |s, a| s.modpack_plan(a.provider, a.project_id, a.version_id).await;
    "modpack_install": ModpackInstallArgs => ProfileView =
        |s, a| s.modpack_install(a.provider, a.project_id, a.version_id, a.name).await;
    "blocked_files": ProfileIdArgs => Vec<BlockedFile> = |s, a| s.blocked_files(a.profile_id);
    "blocked_watch_start": ProfileIdArgs => Vec<String> =
        |s, a| s.blocked_watch_start(a.profile_id).await;
    "blocked_watch_stop": ProfileIdArgs => () = |s, a| s.blocked_watch_stop(a.profile_id);
    "blocked_skip": BlockedSkipArgs => Vec<BlockedFile> =
        |s, a| s.blocked_skip(a.profile_id, a.file_name);
    "blocked_open_folder": NoArgs => String = |s, _a| s.blocked_open_folder();
    "block_icons_status": NoArgs => BlockIconsStatus = |s, _a| Ok(s.block_icons_status());
    "block_icons_prepare": BlockIconsPrepareArgs => BlockIconsStatus =
        |s, a| s.block_icons_prepare(a.force.unwrap_or(false)).await;
    "profile_icon_set_block": ProfileIconSetBlockArgs => ProfileView =
        |s, a| s.profile_icon_set_block(a.id, a.block);
    "profile_icon_set_image": ProfileImageFileArgs => ProfileView =
        |s, a| s.profile_icon_set_image(a.id, a.file_path).await;
    "profile_icon_clear": IdArgs => ProfileView = |s, a| s.profile_icon_clear(a.id);
    "panorama_for_profile": IdArgs => Option<PanoramaSet> =
        |s, a| s.panorama_for_profile(a.id).await;
    "profile_background_set_screenshot": ProfileBackgroundScreenshotArgs => ProfileView =
        |s, a| s.profile_background_set_screenshot(a.id, a.name).await;
    "profile_background_set_image": ProfileImageFileArgs => ProfileView =
        |s, a| s.profile_background_set_image(a.id, a.file_path).await;
    "profile_background_set_style": ProfileBackgroundStyleArgs => ProfileView =
        |s, a| s.profile_background_set_style(a.id, a.dim, a.blur);
    "profile_background_set_kind": ProfileBackgroundKindArgs => ProfileView =
        |s, a| s.profile_background_set_kind(a.id, a.kind, a.wallpaper);
    "profile_icon_use_modpack_logo": IdArgs => ProfileView =
        |s, a| s.profile_icon_use_modpack_logo(a.id).await;
    "profile_background_clear": ProfileBackgroundClearArgs => ProfileView =
        |s, a| s.profile_background_clear(a.id, a.none.unwrap_or(false));
    "pick_image_file": PickArgs => Option<String> = |s, a| s.pick_image_file(a.title).await;
    "pick_import_file": PickArgs => Option<String> = |s, a| s.pick_import_file(a.title).await;
    "pick_folder": PickArgs => Option<String> = |s, a| s.pick_folder(a.title).await;
    "pick_save_file": PickSaveArgs => Option<String> =
        |s, a| s.pick_save_file(a.title, a.default_name).await;
    "worlds_list": ProfileIdArgs => Vec<World> = |s, a| s.worlds_list(a.profile_id).await;
    "world_open_folder": ProfileIdArgs => String = |s, a| s.world_open_folder(a.profile_id);
    "world_backup": WorldBackupArgs => Backup =
        |s, a| s.world_backup(a.profile_id, a.world_id, a.label).await;
    "world_delete": WorldDeleteArgs => Option<Backup> =
        |s, a| s.world_delete(a.profile_id, a.world_id, a.make_backup).await;
    "backups_list": ProfileIdArgs => Vec<Backup> = |s, a| s.backups_list(a.profile_id).await;
    "backup_restore": BackupRestoreArgs => Vec<Backup> =
        |s, a| s.backup_restore(a.profile_id, a.backup_id, a.as_new).await;
    "backup_delete": BackupArgs => Vec<Backup> =
        |s, a| s.backup_delete(a.profile_id, a.backup_id).await;
    "snapshot_create": SnapshotCreateArgs => Backup =
        |s, a| s.snapshot_create(a.profile_id, a.label).await;
    "instance_export": InstanceExportArgs => FileExport = |s, a| {
        s.instance_export(a.profile_id, a.format, a.include_config, a.out_path)
            .await
    };
    "instance_import_plan": InstanceImportPlanArgs => ImportPlan =
        |s, a| s.instance_import_plan(a.file_path).await;
    "instance_import": InstanceImportArgs => ProfileView =
        |s, a| s.instance_import(a.file_path, a.name).await;
    "crash_analyze": CrashAnalyzeArgs => CrashReport =
        |s, a| s.crash_analyze(a.profile_id, a.log_id).await;
    "diagnostics_export": DiagnosticsExportArgs => FileExport =
        |s, a| s.diagnostics_export(a.profile_id, a.out_path).await;
    "servers_list": NoArgs => Vec<ServerEntry> = |s, _a| s.servers_list().await;
    "servers_ping_all": NoArgs => () = |s, _a| s.servers_ping_all().await;
    "servers_ping": AddressArgs => ServerStatus = |s, a| s.servers_ping(a.address).await;
    "screenshots_list": ProfileIdArgs => Vec<Screenshot> =
        |s, a| s.screenshots_list(a.profile_id).await;
    "screenshot_action": ScreenshotActionArgs => () =
        |s, a| s.screenshot_action(a.profile_id, a.name, a.action).await;
    "logs_list": ProfileIdArgs => Vec<LogEntry> = |s, a| s.logs_list(a.profile_id).await;
    "logs_read": LogFileArgs => LogText = |s, a| s.logs_read(a.profile_id, a.id).await;
    "logs_upload": LogFileArgs => LogUpload = |s, a| s.logs_upload(a.profile_id, a.id).await;
    "logs_delete": LogFileArgs => () = |s, a| s.logs_delete(a.profile_id, a.id).await;
    "stats_summary": StatsSummaryArgs => StatsSummary = |s, a| s.stats_summary(a.days);
    "storage_usage": NoArgs => Vec<StorageUsage> = |s, _a| s.storage_usage().await;
    "storage_clear": StorageClearArgs => Vec<StorageUsage> = |s, a| s.storage_clear(a.key).await;
    "update_status": NoArgs => UpdateStatus = |s, _a| Ok(s.update_status());
    "update_check": NoArgs => UpdateStatus = |s, _a| s.update_check().await;
    "update_install": NoArgs => bool = |s, _a| s.update_install();
}
