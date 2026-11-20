import { api as defaultApi, toAppError, type Api, type Unlisten } from './api';
import { classifyLoginError, isAllowedExternalUrl, type LoginErrorInfo } from './auth';
import { DEFAULT_LANG, hasKey, plural, translate, type Key, type Lang, type PluralKey, type Vars } from './i18n';
import type {
  AppState, Config, InstallProgressEvent, LaunchProgressEvent, LaunchState, LogLevel, ProfileView, ProjectType, PublicAccount,
  ServerEntry, ServerStatus, UpdateStatus,
} from './types';

export const CONSOLE_CAP = 800;

export const LAUNCH_STAGES = ['preparing', 'java', 'loader', 'downloading', 'account', 'launching', 'running'] as const;
export type Stage = (typeof LAUNCH_STAGES)[number];

export interface ConsoleLine { id: number; line: string; level: LogLevel }
export interface LaunchInfo {
  state: LaunchState | 'idle';
  progress: LaunchProgressEvent | null;
  /** 0..1 of the current "downloading" stage, from launch:progress. */
  downloadFraction: number;
}

export const app = $state({
  ready: false,
  version: '',
  config: null as Config | null,
  profiles: [] as ProfileView[],
  accounts: [] as PublicAccount[],
  totalRamMb: 0,
  rootDir: '',
  error: null as { code: string; message: string } | null,
});

export const launches = $state<Record<string, LaunchInfo>>({});
export const consoles = $state<Record<string, ConsoleLine[]>>({});
export type View = 'home' | 'library' | 'discover' | 'servers' | 'stats' | 'settings' | 'profile';
export type ProfileTab = 'overview' | 'content' | 'media' | 'logs' | 'settings';
export const nav = $state({
  view: 'home' as View, newProfile: false, accounts: false,
  /** Profile page target and tab. */
  profileId: null as string | null, profileTab: 'overview' as ProfileTab,
  /** Discover scope: the profile content is installed into (null = no scope), and the initial project type. */
  discoverProfile: null as string | null, discoverType: 'mod' as ProjectType,
});

export function openProfile(id: string, tab: ProfileTab = 'overview'): void {
  nav.profileId = id; nav.profileTab = tab; nav.view = 'profile';
}
export function openDiscover(profileId: string | null = null, type: ProjectType = 'mod'): void {
  nav.discoverProfile = profileId ?? selectedProfile()?.id ?? null;
  nav.discoverType = type; nav.view = 'discover';
}

/* ---- toasts ---- */
export interface Toast { id: number; text: string; kind: 'info' | 'ok' | 'error' }
export const toasts = $state<Toast[]>([]);
let toastId = 0;
export function toast(text: string, kind: Toast['kind'] = 'info', ms = 4500): void {
  const id = ++toastId;
  toasts.push({ id, text, kind });
  setTimeout(() => dismissToast(id), ms);
}
export function dismissToast(id: number): void {
  const i = toasts.findIndex((x) => x.id === id);
  if (i >= 0) toasts.splice(i, 1);
}

/* ---- part 3 state: Microsoft login ---- */
export const auth = $state({
  /** The login window is open (or the request is in flight). */
  signingIn: false,
  error: null as LoginErrorInfo | null,
  /** Set when game_launch failed with needsRelogin; the dialog retries this launch after a successful login. */
  relogin: null as { profileId: string; quickJoin?: string } | null,
  /** The "login lasts only this session" notice was dismissed (in memory only, so it returns next start). */
  noticeDismissed: false,
  refreshing: {} as Record<string, boolean>,
});
/** True when some account's login is memory-only because no system keystore exists. */
export const keystoreMissing = (): boolean => app.accounts.some((a) => a.credentialsPersisted === false);
export const accountNeedsLogin = (a: PublicAccount): boolean => a.type === 'microsoft' && (a.needsRelogin || a.expired);

/* ---- part 2 state ---- */
export const servers = $state({ list: [] as ServerEntry[], status: {} as Record<string, ServerStatus>, loaded: false, pinging: false });
/** Running installs keyed by projectId (modpacks have no profile yet, so projectId is the only stable key). */
export const installs = $state<Record<string, InstallProgressEvent>>({});
/** Bumped when a profile's content folder changed on disk; content views watch it. */
export const contentRev = $state<Record<string, number>>({});
export const updates = $state({ status: { state: 'idle' } as UpdateStatus, overlay: false, installing: false });

let lineId = 0;
let currentApi: Api = defaultApi;
let unlisteners: Unlisten[] = [];

export const lang = (): Lang => app.config?.language ?? DEFAULT_LANG;
/** Reactive translate: reads the language rune, so templates re-render on change. */
export const t = (key: Key, vars?: Vars): string => translate(lang(), key, vars);
export const tp = (key: PluralKey, n: number, vars?: Vars): string => plural(lang(), key, n, vars);
export function errorText(e: unknown): string {
  const code = toAppError(e).code;
  const key = `err.${code}`;
  return hasKey(lang(), key) ? translate(lang(), key) : translate(lang(), 'err.unknown');
}

export const selectedProfile = (): ProfileView | null =>
  app.profiles.find((p) => p.id === app.config?.selectedProfile) ?? app.profiles[0] ?? null;
export const selectedAccount = (): PublicAccount | null =>
  app.accounts.find((a) => a.uuid === app.config?.selectedAccount) ?? app.accounts[0] ?? null;

export function launchInfo(id: string): LaunchInfo {
  return launches[id] ?? { state: 'idle', progress: null, downloadFraction: 0 };
}
export const isBusy = (id: string): boolean => {
  const s = launchInfo(id).state;
  return s !== 'idle' && s !== 'stopped' && s !== 'crashed';
};
/** True while any game is starting or running (storage clearing is refused then). */
export const anyGameActive = (): boolean => Object.keys(launches).some((id) => isBusy(id));
export const isRunning = (id: string): boolean => launchInfo(id).state === 'running';

/** 0..1 across the staged bar. Downloading interpolates with progress events. */
export function stageProgress(info: LaunchInfo): number {
  if (info.state === 'idle' || info.state === 'stopped' || info.state === 'crashed') return 0;
  const i = LAUNCH_STAGES.indexOf(info.state as Stage);
  if (i < 0) return 0;
  if (info.state === 'running') return 1;
  const within = info.state === 'downloading' ? info.downloadFraction : 0;
  return (i + within) / LAUNCH_STAGES.length;
}

export function applyState(profileId: string, state: LaunchState): void {
  const prev = launches[profileId];
  launches[profileId] = {
    state,
    progress: state === 'downloading' ? prev?.progress ?? null : null,
    downloadFraction: state === 'downloading' ? prev?.downloadFraction ?? 0 : 0,
  };
}

export function applyProgress(ev: LaunchProgressEvent): void {
  const prev = launches[ev.profileId] ?? { state: 'idle' as const, progress: null, downloadFraction: 0 };
  let f = prev.downloadFraction;
  if (ev.phase === 'client' || ev.phase === 'libraries' || ev.phase === 'assets') {
    const idx = { client: 0, libraries: 1, assets: 2 }[ev.phase];
    f = Math.max(f, (idx + (ev.total > 0 ? ev.done / ev.total : 1)) / 3);
  } else if (ev.phase === 'done') f = 1;
  launches[ev.profileId] = { ...prev, progress: ev, downloadFraction: Math.min(1, f) };
}

export function pushLog(profileId: string, line: string, level: LogLevel): void {
  const buf = consoles[profileId] ?? (consoles[profileId] = []);
  buf.push({ id: ++lineId, line, level });
  if (buf.length > CONSOLE_CAP) buf.splice(0, buf.length - CONSOLE_CAP);
}
export function clearConsole(profileId: string): void {
  consoles[profileId] = [];
}

function applyAppState(s: AppState): void {
  app.version = s.version;
  app.config = s.config;
  app.profiles = s.profiles;
  app.accounts = s.accounts;
  app.totalRamMb = s.totalRamMb;
  app.rootDir = s.rootDir;
  for (const id of s.running) if (!isBusy(id)) applyState(id, 'running');
}

export async function init(api: Api = defaultApi): Promise<void> {
  for (const u of unlisteners) u();
  unlisteners = [];
  currentApi = api;
  unlisteners = await Promise.all([
    api.listen('launch:state', (e) => applyState(e.profileId, e.state)),
    api.listen('launch:progress', applyProgress),
    api.listen('launch:log', (e) => pushLog(e.profileId, e.line, e.level)),
    api.listen('install:progress', applyInstallProgress),
    api.listen('servers:pinged', applyPinged),
    api.listen('update:status', (u) => { updates.status = u; }),
    api.listen('content:changed', (e) => { contentRev[e.profileId] = (contentRev[e.profileId] ?? 0) + 1; }),
    api.listen('accounts:changed', () => { void refreshAccounts(); }),
  ]);
  try {
    applyAppState(await api.appState());
    app.error = null;
    api.updateStatus().then((u) => { updates.status = u; }, () => {});
  } catch (e) {
    const err = toAppError(e);
    app.error = { code: err.code, message: err.message };
  }
  app.ready = true;
}

export function applyInstallProgress(e: InstallProgressEvent): void {
  if (e.done >= e.total) delete installs[e.projectId];
  else installs[e.projectId] = e;
}
export function applyPinged(s: ServerStatus): void {
  servers.status[s.id] = s;
}

export const getApi = (): Api => currentApi;

export function resetStores(): void {
  app.ready = false; app.config = null; app.profiles = []; app.accounts = []; app.error = null;
  for (const k of Object.keys(launches)) delete launches[k];
  for (const k of Object.keys(consoles)) delete consoles[k];
  nav.view = 'home'; nav.newProfile = false; nav.accounts = false; nav.profileId = null; nav.profileTab = 'overview';
  nav.discoverProfile = null; nav.discoverType = 'mod';
  toasts.length = 0; servers.list = []; servers.status = {}; servers.loaded = false; servers.pinging = false;
  for (const k of Object.keys(installs)) delete installs[k];
  for (const k of Object.keys(contentRev)) delete contentRev[k];
  updates.status = { state: 'idle' }; updates.overlay = false; updates.installing = false;
  auth.signingIn = false; auth.error = null; auth.relogin = null; auth.noticeDismissed = false;
  for (const k of Object.keys(auth.refreshing)) delete auth.refreshing[k];
}

/* ---- actions (thin wrappers; logic stays in Rust) ---- */
async function guard<T>(fn: () => Promise<T>): Promise<T | undefined> {
  try { return await fn(); } catch (e) {
    const err = toAppError(e);
    app.error = { code: err.code, message: err.message };
    return undefined;
  }
}
export const actions = {
  async setConfig(patch: Partial<Config>) {
    if (app.config) Object.assign(app.config, patch); // optimistic
    const c = await guard(() => currentApi.configSet({ patch }));
    if (c) app.config = c;
  },
  async selectProfile(id: string) {
    const c = await guard(() => currentApi.profileSelect({ id }));
    if (c) app.config = c;
  },
  async selectAccount(uuid: string) {
    const c = await guard(() => currentApi.accountSelect({ uuid }));
    if (c) app.config = c;
  },
  async addOffline(name: string) {
    const a = await currentApi.accountAddOffline({ name }); // caller handles validation error
    app.accounts = [...app.accounts, a];
    if (!app.config?.selectedAccount) await actions.selectAccount(a.uuid);
    return a;
  },
  async removeAccount(uuid: string) {
    await guard(() => currentApi.accountRemove({ uuid }));
    await refresh();
  },
  /**
   * Opens the Microsoft login. Resolves the signed-in account, `null` when the user cancelled (silent), or `undefined` on a
   * failure (then `auth.error` holds a localized, classified error).
   */
  async signInMicrosoft(): Promise<PublicAccount | null | undefined> {
    if (auth.signingIn) return undefined;
    auth.signingIn = true; auth.error = null;
    try {
      const a = await currentApi.accountSignInMicrosoft();
      if (!a) return null;
      app.accounts = app.accounts.some((x) => x.uuid === a.uuid) ? app.accounts.map((x) => (x.uuid === a.uuid ? a : x)) : [...app.accounts, a];
      await actions.selectAccount(a.uuid);
      return a;
    } catch (e) {
      auth.error = classifyLoginError(e);
      return undefined;
    } finally {
      auth.signingIn = false;
    }
  },
  async refreshAccount(uuid: string) {
    auth.refreshing[uuid] = true;
    try {
      const a = await currentApi.accountRefresh({ uuid });
      app.accounts = app.accounts.map((x) => (x.uuid === uuid ? a : x));
      return a;
    } catch (e) {
      // A dead refresh token is not a failure to report, the card just switches to "Sign in again".
      await refreshAccounts();
      if (!toAppError(e).needsRelogin) toast(errorText(e), 'error');
      return undefined;
    } finally {
      delete auth.refreshing[uuid];
    }
  },
  /** Login from the "session expired" dialog; on success the failed launch is retried automatically. */
  async reloginAndRetry() {
    const pending = auth.relogin;
    if (!pending) return;
    const a = await actions.signInMicrosoft();
    if (!a || auth.relogin !== pending) return; // cancelled, failed (error stays visible) or dialog closed meanwhile
    auth.relogin = null;
    await actions.play(pending.profileId, pending.quickJoin);
  },
  openExternal(url: string) {
    if (!isAllowedExternalUrl(url)) return Promise.resolve();
    return currentApi.openExternal({ url }).catch(() => {});
  },
  async createProfile(a: { name: string; mcVersion: string; loader: ProfileView['loader']; loaderVersion?: string | null }, extra: Partial<ProfileView>) {
    let p = await currentApi.profileCreate(a);
    if (Object.keys(extra).length) p = await currentApi.profileUpdate({ id: p.id, patch: extra });
    app.profiles = [...app.profiles, p];
    await actions.selectProfile(p.id);
    return p;
  },
  async play(id: string, quickJoin?: string) {
    clearConsole(id);
    applyState(id, 'preparing');
    try {
      await currentApi.gameLaunch(quickJoin ? { profileId: id, quickJoin } : { profileId: id });
    } catch (e) {
      const err = toAppError(e);
      if (err.needsRelogin) {
        applyState(id, 'stopped');
        auth.error = null;
        auth.relogin = quickJoin ? { profileId: id, quickJoin } : { profileId: id };
        return;
      }
      app.error = { code: err.code, message: err.message };
      applyState(id, 'crashed');
    }
  },
  async updateProfile(id: string, patch: Partial<ProfileView>) {
    const p = await currentApi.profileUpdate({ id, patch }); // caller handles errors
    app.profiles = app.profiles.map((x) => (x.id === id ? p : x));
    return p;
  },
  async deleteProfile(id: string) {
    await currentApi.profileDelete({ id });
    await refresh();
    if (nav.profileId === id) { nav.profileId = null; nav.view = 'library'; }
  },
  async addProfile(p: ProfileView) {
    app.profiles = [...app.profiles.filter((x) => x.id !== p.id), p];
    await actions.selectProfile(p.id);
  },
  async loadServers(force = false) {
    if (servers.loaded && !force) return;
    const l = await guard(() => currentApi.serversList());
    if (!l) return;
    servers.list = l; servers.loaded = true;
    await actions.pingServers();
  },
  async pingServers() {
    servers.pinging = true;
    await guard(() => currentApi.serversPingAll());
    setTimeout(() => { servers.pinging = false; }, 2500);
  },
  async updateCheck() {
    const u = await guard(() => currentApi.updateCheck());
    if (u) updates.status = u;
  },
  async updateInstall() {
    updates.installing = true;
    const ok = await guard(() => currentApi.updateInstall());
    if (!ok) { updates.installing = false; updates.overlay = false; }
  },
  async stop(id: string) { await guard(() => currentApi.gameStop({ profileId: id })); },
};

/** Re-reads the account list (accounts:changed); keeps everything else untouched. */
export async function refreshAccounts(): Promise<void> {
  const s = await guard(() => currentApi.appState());
  if (!s) return;
  app.accounts = s.accounts;
  if (app.config) app.config.selectedAccount = s.config.selectedAccount;
}

export async function refresh(): Promise<void> {
  const s = await guard(() => currentApi.appState());
  if (s) applyAppState(s);
}
