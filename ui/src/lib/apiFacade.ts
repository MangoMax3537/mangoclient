// One typed facade over the generated command table. `COMMANDS` maps the UI method names to the Rust command names; everything else
// (argument and result types, optional arguments for argument-less commands) is derived from `CommandMap`. A transport only has to
// provide `invoke` and `listen`: Tauri, the mango-dev HTTP bridge and the tests all plug in here.
import type { CommandMap, CommandName, EventMap, EventName } from './generated';

export type Unlisten = () => void;

/** The one function every backend implements. */
export type Invoke = <K extends CommandName>(command: K, args: CommandMap[K]['args']) => Promise<CommandMap[K]['result']>;
export type Subscribe = <K extends EventName>(name: K, cb: (payload: EventMap[K]) => void) => Promise<Unlisten>;
export interface Transport { invoke: Invoke; listen: Subscribe }

/** UI method name -> Rust command. */
export const COMMANDS = {
  appState: 'app_state',
  configSet: 'config_set',
  profileCreate: 'profile_create',
  profileUpdate: 'profile_update',
  profileDelete: 'profile_delete',
  profileSelect: 'profile_select',
  profileDuplicate: 'profile_duplicate',
  accountAddOffline: 'account_add_offline',
  accountSelect: 'account_select',
  accountRemove: 'account_remove',
  accountSignInMicrosoft: 'account_sign_in_microsoft',
  accountRefresh: 'account_refresh',
  versionsManifest: 'versions_manifest',
  loaderVersions: 'loader_versions',
  gameLaunch: 'game_launch',
  gameStop: 'game_stop',
  gameRunning: 'game_running',
  openFolder: 'open_folder',
  openExternal: 'open_external',
  contentList: 'content_list',
  contentToggle: 'content_toggle',
  contentUninstall: 'content_uninstall',
  contentCheckUpdates: 'content_check_updates',
  contentOpenFolder: 'content_open_folder',
  searchProjects: 'search_projects',
  projectDetail: 'project_detail',
  projectVersions: 'project_versions',
  installTargets: 'install_targets',
  installPlan: 'install_plan',
  installApply: 'install_apply',
  modpackPlan: 'modpack_plan',
  modpackInstall: 'modpack_install',
  blockIconsStatus: 'block_icons_status',
  blockIconsPrepare: 'block_icons_prepare',
  profileIconSetBlock: 'profile_icon_set_block',
  profileIconSetImage: 'profile_icon_set_image',
  profileIconClear: 'profile_icon_clear',
  panoramaForProfile: 'panorama_for_profile',
  profileBackgroundSetScreenshot: 'profile_background_set_screenshot',
  profileBackgroundSetImage: 'profile_background_set_image',
  profileBackgroundClear: 'profile_background_clear',
  serversList: 'servers_list',
  serversPingAll: 'servers_ping_all',
  serversPing: 'servers_ping',
  screenshotsList: 'screenshots_list',
  screenshotAction: 'screenshot_action',
  logsList: 'logs_list',
  logsRead: 'logs_read',
  logsUpload: 'logs_upload',
  logsDelete: 'logs_delete',
  statsSummary: 'stats_summary',
  storageUsage: 'storage_usage',
  storageClear: 'storage_clear',
  updateStatus: 'update_status',
  updateCheck: 'update_check',
  updateInstall: 'update_install',
  profileBackgroundSetStyle: 'profile_background_set_style',
  profileBackgroundSetKind: 'profile_background_set_kind',
  profileIconUseModpackLogo: 'profile_icon_use_modpack_logo',
  pickImageFile: 'pick_image_file',
  pickFolder: 'pick_folder',
  pickSaveFile: 'pick_save_file',
  pickImportFile: 'pick_import_file',
  worldsList: 'worlds_list',
  worldOpenFolder: 'world_open_folder',
  worldBackup: 'world_backup',
  worldDelete: 'world_delete',
  backupsList: 'backups_list',
  backupRestore: 'backup_restore',
  backupDelete: 'backup_delete',
  snapshotCreate: 'snapshot_create',
  instanceExport: 'instance_export',
  instanceImportPlan: 'instance_import_plan',
  instanceImport: 'instance_import',
  crashAnalyze: 'crash_analyze',
  diagnosticsExport: 'diagnostics_export',
  skinGet: 'skin_get',
  skinUpload: 'skin_upload',
  skinUseUsername: 'skin_use_username',
  skinSetCape: 'skin_set_cape',
  skinLibraryList: 'skin_library_list',
  skinLibraryAdd: 'skin_library_add',
  skinLibrarySaveCurrent: 'skin_library_save_current',
  skinLibraryRemove: 'skin_library_remove',
  skinLibraryRename: 'skin_library_rename',
  skinLibraryApply: 'skin_library_apply',
  javaList: 'java_list',
  javaInstall: 'java_install',
  panoramaPool: 'panorama_pool',
  windowMinimize: 'window_minimize',
  windowToggleMaximize: 'window_toggle_maximize',
  windowClose: 'window_close',
  windowIsMaximized: 'window_is_maximized',
  windowStartDrag: 'window_start_drag',
  windowStartResize: 'window_start_resize',
} as const satisfies Record<string, CommandName>;

/**
 * Commands the Rust side keeps as aliases for one release (docs/ipc.md Part 4). The UI uses the new names, so they have no method.
 */
export const DEPRECATED_COMMANDS = [
  'modrinth_search', 'modrinth_project', 'modrinth_versions', 'modrinth_install', 'modrinth_install_modpack',
] as const satisfies readonly CommandName[];

type Methods = typeof COMMANDS;
type Covered = Methods[keyof Methods] | (typeof DEPRECATED_COMMANDS)[number];
/** Compile error ("Type 'x' is not assignable to type 'never'") naming every Rust command that has no UI method. */
const _everyCommandHasAMethod: Exclude<CommandName, Covered> extends never ? true : Exclude<CommandName, Covered> = true;
void _everyCommandHasAMethod;

/** Unit results travel as `null` on the wire; the facade exposes them as `void`. */
type Result<K extends CommandName> = [CommandMap[K]['result']] extends [null] ? void : CommandMap[K]['result'];
type Call<K extends CommandName> = {} extends CommandMap[K]['args']
  ? (args?: CommandMap[K]['args']) => Promise<Result<K>>
  : (args: CommandMap[K]['args']) => Promise<Result<K>>;

/** One method per command, plus a typed event subscription. */
export type Api = { [M in keyof Methods]: Call<Methods[M]> } & { listen: Subscribe };

/** Builds the method facade over a transport. Arguments default to `{}` (the dispatch treats an empty object as "no arguments"). */
export function createApi(transport: Transport): Api {
  const methods: Record<string, unknown> = { listen: transport.listen };
  for (const [method, command] of Object.entries(COMMANDS)) {
    methods[method] = (args?: unknown) => transport.invoke(command, (args ?? {}) as never);
  }
  return methods as Api;
}
