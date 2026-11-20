import { invoke } from '@tauri-apps/api/core';
import { listen as tauriListen } from '@tauri-apps/api/event';
import type {
  ApiEvents, AppError, AppState, Config, LoaderVersion, Loader, ProfileView, PublicAccount, VersionManifest,
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
  listen: (name, cb) => tauriListen<never>(name, (e) => cb(e.payload as never)),
};

export { createMockApi };
export const mockApi: Api = createMockApi();

export function inTauri(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}

/** Tauri when running inside the shell, the in-memory mock in a plain browser. */
export const api: Api = inTauri() ? tauriApi : mockApi;

export function toAppError(e: unknown): AppError {
  if (e && typeof e === 'object' && 'code' in e && typeof (e as AppError).code === 'string') return e as AppError;
  return { code: 'unknown', message: e instanceof Error ? e.message : String(e), needsRelogin: false };
}
