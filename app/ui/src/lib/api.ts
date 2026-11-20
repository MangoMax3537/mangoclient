import { invoke } from '@tauri-apps/api/core';
import { listen as tauriListen } from '@tauri-apps/api/event';
import type {
  ApiEvents, AppError, AppState, Config, LoaderVersion, Loader, ProfileView, PublicAccount, VersionManifest,
  ContentItem, ContentType, LogEntry, ProjectDetail, ProjectVersion, Screenshot, SearchParams, SearchResult, ServerEntry,
  ServerStatus, StatsSummary, StorageKey, StorageUsage, UpdateInfo, UpdateStatus,
} from './types';
import { createMockApi } from './mockApi';

export type Unlisten = () => void;

/** One method per command in docs/ipc.md, plus a typed event subscription. */
export interface Api {
  appState(): Promise<AppState>;
  configSet(args: { patch: Partial<Config> }): Promise<Config>;
  profileCreate(args: { name: string; mcVersion: string; loader: Loader; loaderVersion?: string | null }): Promise<ProfileView>;
  profileUpdate(args: { id: string; patch: Partial<ProfileView> }): Promise<ProfileView>;
  profileDelete(args: { id: string }): Promise<void>;
  profileSelect(args: { id: string }): Promise<Config>;
  profileDuplicate(args: { id: string }): Promise<ProfileView>;
  accountAddOffline(args: { name: string }): Promise<PublicAccount>;
  accountSelect(args: { uuid: string }): Promise<Config>;
  accountRemove(args: { uuid: string }): Promise<void>;
  accountSignInMicrosoft(): Promise<PublicAccount | null>;
  versionsManifest(args?: { force?: boolean }): Promise<VersionManifest>;
  loaderVersions(args: { loader: Loader; mcVersion: string }): Promise<LoaderVersion[]>;
  gameLaunch(args: { profileId: string; quickJoin?: string }): Promise<{ pid: number; versionId: string }>;
  gameStop(args: { profileId: string }): Promise<boolean>;
  gameRunning(): Promise<string[]>;
  openFolder(args: { target: 'root' | 'logs' | string }): Promise<string>;
  /* Part 2 */
  contentList(args: { profileId: string }): Promise<ContentItem[]>;
  contentToggle(args: { profileId: string; projectId: string; enabled: boolean }): Promise<{ items: ContentItem[]; affected: string[] }>;
  contentUninstall(args: { profileId: string; projectId: string }): Promise<{ blocked: boolean; dependents: string[]; items: ContentItem[] }>;
  contentCheckUpdates(args: { profileId: string }): Promise<UpdateInfo[]>;
  contentOpenFolder(args: { profileId: string; type: ContentType }): Promise<string>;
  modrinthSearch(args: { params: SearchParams }): Promise<SearchResult>;
  modrinthProject(args: { projectId: string }): Promise<ProjectDetail>;
  modrinthVersions(args: { projectId: string; profileId?: string }): Promise<ProjectVersion[]>;
  modrinthInstall(args: { profileId: string; projectId: string; versionId?: string }): Promise<ContentItem[]>;
  modrinthInstallModpack(args: { projectId: string; versionId?: string; name?: string }): Promise<ProfileView>;
  serversList(): Promise<ServerEntry[]>;
  serversPingAll(): Promise<void>;
  serversPing(args: { address: string }): Promise<ServerStatus>;
  screenshotsList(args: { profileId: string }): Promise<Screenshot[]>;
  screenshotAction(args: { profileId: string; name: string; action: 'delete' | 'reveal' | 'copy' }): Promise<void>;
  logsList(args: { profileId: string }): Promise<LogEntry[]>;
  logsRead(args: { profileId: string; id: string }): Promise<{ text: string; truncated: boolean }>;
  logsUpload(args: { profileId: string; id: string }): Promise<{ url: string }>;
  logsDelete(args: { profileId: string; id: string }): Promise<void>;
  statsSummary(args: { days: number }): Promise<StatsSummary>;
  storageUsage(): Promise<StorageUsage[]>;
  storageClear(args: { key: StorageKey }): Promise<StorageUsage[]>;
  updateStatus(): Promise<UpdateStatus>;
  updateCheck(): Promise<UpdateStatus>;
  updateInstall(): Promise<boolean>;
  listen<K extends keyof ApiEvents>(name: K, cb: (payload: ApiEvents[K]) => void): Promise<Unlisten>;
}

export const tauriApi: Api = {
  appState: () => invoke('app_state'),
  configSet: (a) => invoke('config_set', a),
  profileCreate: (a) => invoke('profile_create', a),
  profileUpdate: (a) => invoke('profile_update', a),
  profileDelete: (a) => invoke('profile_delete', a),
  profileSelect: (a) => invoke('profile_select', a),
  profileDuplicate: (a) => invoke('profile_duplicate', a),
  accountAddOffline: (a) => invoke('account_add_offline', a),
  accountSelect: (a) => invoke('account_select', a),
  accountRemove: (a) => invoke('account_remove', a),
  accountSignInMicrosoft: () => invoke('account_sign_in_microsoft'),
  versionsManifest: (a) => invoke('versions_manifest', a ?? {}),
  loaderVersions: (a) => invoke('loader_versions', a),
  gameLaunch: (a) => invoke('game_launch', a),
  gameStop: (a) => invoke('game_stop', a),
  gameRunning: () => invoke('game_running'),
  openFolder: (a) => invoke('open_folder', a),
  contentList: (a) => invoke('content_list', a),
  contentToggle: (a) => invoke('content_toggle', a),
  contentUninstall: (a) => invoke('content_uninstall', a),
  contentCheckUpdates: (a) => invoke('content_check_updates', a),
  contentOpenFolder: (a) => invoke('content_open_folder', a),
  modrinthSearch: (a) => invoke('modrinth_search', a),
  modrinthProject: (a) => invoke('modrinth_project', a),
  modrinthVersions: (a) => invoke('modrinth_versions', a),
  modrinthInstall: (a) => invoke('modrinth_install', a),
  modrinthInstallModpack: (a) => invoke('modrinth_install_modpack', a),
  serversList: () => invoke('servers_list'),
  serversPingAll: () => invoke('servers_ping_all'),
  serversPing: (a) => invoke('servers_ping', a),
  screenshotsList: (a) => invoke('screenshots_list', a),
  screenshotAction: (a) => invoke('screenshot_action', a),
  logsList: (a) => invoke('logs_list', a),
  logsRead: (a) => invoke('logs_read', a),
  logsUpload: (a) => invoke('logs_upload', a),
  logsDelete: (a) => invoke('logs_delete', a),
  statsSummary: (a) => invoke('stats_summary', a),
  storageUsage: () => invoke('storage_usage'),
  storageClear: (a) => invoke('storage_clear', a),
  updateStatus: () => invoke('update_status'),
  updateCheck: () => invoke('update_check'),
  updateInstall: () => invoke('update_install'),
  listen: (name, cb) => tauriListen<never>(name, (e) => cb(e.payload as never)),
};

export { createMockApi };
const firstRun = typeof location !== 'undefined' && new URLSearchParams(location.search).has('onboarding');
export const mockApi: Api = createMockApi({ firstRun });

export function inTauri(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}

/** Tauri when running inside the shell, the in-memory mock in a plain browser. */
export const api: Api = inTauri() ? tauriApi : mockApi;

export function toAppError(e: unknown): AppError {
  if (e && typeof e === 'object' && 'code' in e && typeof (e as AppError).code === 'string') return e as AppError;
  return { code: 'unknown', message: e instanceof Error ? e.message : String(e), needsRelogin: false };
}
