import { invoke } from '@tauri-apps/api/core';
import { listen as tauriListen } from '@tauri-apps/api/event';
import type { AppError } from './types';
import { createApi, type Api, type Unlisten } from './apiFacade';
import { backendFromSearch, createHttpApi } from './httpApi';
import { createMockApi, mockOptionsFromSearch, type MockApi } from './mockApi';

export { createApi, COMMANDS, DEPRECATED_COMMANDS } from './apiFacade';
export type { Api, Invoke, Subscribe, Transport, Unlisten } from './apiFacade';

/** Tauri transport: `invoke` and `listen` from the shell, nothing else. */
export const tauriApi: Api = createApi({
  invoke: (command, args) => invoke(command, args as Record<string, unknown>),
  listen: (name, cb) => tauriListen<never>(name, (e) => cb(e.payload as never)) as Promise<Unlisten>,
});

export { createMockApi };
export const mockApi: MockApi = createMockApi(mockOptionsFromSearch(typeof location !== 'undefined' ? location.search : ''));

export function inTauri(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}

export type BackendKind = 'tauri' | 'http' | 'mock';

/** Tauri inside the shell, the mango-dev bridge for `?backend=...&token=...`, the in-memory mock in a plain browser. */
export function backendKind(search: string, tauri: boolean): BackendKind {
  return tauri ? 'tauri' : backendFromSearch(search) ? 'http' : 'mock';
}
const search = typeof location !== 'undefined' ? location.search : '';
export const kind: BackendKind = backendKind(search, inTauri());
export const inHttp = (): boolean => kind === 'http';
const httpParams = kind === 'http' ? backendFromSearch(search) : null;
export const api: Api = kind === 'tauri' ? tauriApi : httpParams ? createHttpApi(httpParams) : mockApi;

export function toAppError(e: unknown): AppError {
  if (e && typeof e === 'object' && 'code' in e && typeof (e as AppError).code === 'string') return e as AppError;
  return { code: 'unknown', message: e instanceof Error ? e.message : String(e), needsRelogin: false };
}

// Dev helper for the browser mock: `__mangoMock.download(profileId)` plays the user saving a blocked file into Downloads.
if (kind === 'mock' && typeof window !== 'undefined') (window as unknown as { __mangoMock: MockApi['dev'] }).__mangoMock = mockApi.dev;
