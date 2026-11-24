import { invoke } from '@tauri-apps/api/core';
import { listen as tauriListen } from '@tauri-apps/api/event';
import type { AppError } from './types';
import { createApi, type Api, type Unlisten } from './apiFacade';
import { backendFromSearch, createHttpApi } from './httpApi';
import type { MockApi } from './mockApi';
import { mapStrings, servesSchemeOnLocalhost, toLocalhostImg, toRawImg } from './imgScheme';

export { createApi, COMMANDS, DEPRECATED_COMMANDS } from './apiFacade';
export type { Api, Invoke, Subscribe, Transport, Unlisten } from './apiFacade';

// Windows (WebView2): images load from http(s)://mangoimg.localhost, see imgScheme.ts.
const localhostImg = typeof navigator !== 'undefined' && servesSchemeOnLocalhost(navigator.userAgent);
const httpsPage = typeof location !== 'undefined' && location.protocol === 'https:';
const inbound = <T,>(v: T): T => (localhostImg ? mapStrings(v, (s) => toLocalhostImg(s, httpsPage)) : v);
const outbound = <T,>(v: T): T => (localhostImg ? mapStrings(v, toRawImg) : v);

/** Tauri transport: `invoke` and `listen` from the shell (image URLs adapted to the platform's webview). */
export const tauriApi: Api = createApi({
  invoke: async (command, args) => inbound(await invoke(command, outbound(args) as Record<string, unknown>)),
  listen: (name, cb) => tauriListen<never>(name, (e) => cb(inbound(e.payload) as never)) as Promise<Unlisten>,
});

/**
 * The browser mock (hundreds of KB of fake catalogue data) is only needed without the shell or a dev server, so it is a separate chunk:
 * this facade loads it on the first call. Tauri and `?backend=` builds never fetch it.
 */
export function lazyMockApi(search: string): Api {
  let impl: Promise<MockApi> | null = null;
  const load = (): Promise<MockApi> => (impl ??= import('./mockApi').then((m) => {
    const a = m.createMockApi(m.mockOptionsFromSearch(search));
    // Dev helper of the browser mock: `__mangoMock.download(profileId)` `.drop(paths)` plays files:dropped.
    if (typeof window !== 'undefined') (window as unknown as { __mangoMock: MockApi['dev'] }).__mangoMock = a.dev;
    return a;
  }));
  return new Proxy({} as Api, {
    get: (_t, key) => (...args: unknown[]) => load().then((a) => (a as unknown as Record<string | symbol, (...x: unknown[]) => unknown>)[key](...args)),
  });
}

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
export const api: Api = kind === 'tauri' ? tauriApi : httpParams ? createHttpApi(httpParams) : lazyMockApi(search);

export function toAppError(e: unknown): AppError {
  if (e && typeof e === 'object' && 'code' in e && typeof (e as AppError).code === 'string') return e as AppError;
  return { code: 'unknown', message: e instanceof Error ? e.message : String(e), needsRelogin: false };
}
