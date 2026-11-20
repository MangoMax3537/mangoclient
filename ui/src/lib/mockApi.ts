import type {
  Api, Unlisten,
} from './api';
import { createContentApi } from './mockContent';
import { createIconsMock, type MockBlocksMode, type MockPanoramaMode } from './mockIcons';
import { createBlockedMock, type BlockedDev } from './mockBlocked';
import { fallbackImage } from './fallbackImage';
import type {
  ApiEvents, AppState, Config, LaunchState, LoaderVersion, ProfileView, PublicAccount, VersionManifest, VersionEntry, Loader,
} from './types';

export interface MockOptions {
  /** Multiplies every simulated delay. 1 = about 6 s per launch. */
  timeScale?: number;
  /** Start with config.firstRunDone=false so the onboarding shows. */
  firstRun?: boolean;
  /** What `accountSignInMicrosoft` does: succeed (default), resolve null (cancelled) or fail with a login error variant. */
  login?: 'success' | 'cancel' | { error: string };
  /** Simulate a system without keystore: Microsoft accounts are memory-only. */
  noKeystore?: boolean;
  /** The seeded Microsoft account has no usable refresh token (launch then fails with needsRelogin). */
  needsRelogin?: boolean;
  /** The seeded Microsoft account has a locked credential store. */
  locked?: boolean;
  /** No CurseForge API key: providers_status reports it unavailable and every CurseForge command fails. */
  noCurseforge?: boolean;
  /** Blocked files appear in the Downloads folder by themselves, one per watcher tick. */
  blockedAuto?: boolean;
  /** Part 6: how the block gallery behaves (see mockIcons). */
  blocks?: MockBlocksMode;
  /** Part 7: `off` = no panorama is ever extracted. */
  panorama?: MockPanoramaMode;
}

/** Error code per `?mockLoginError=<variant>` (codes from crates/auth/src/error.rs). */
export const MOCK_LOGIN_ERRORS: Record<string, string> = {
  no_xbox_profile: 'auth.xbox.no_profile', country: 'auth.xbox.country', adult_verification: 'auth.xbox.adult_verification',
  child_account: 'auth.xbox.child_account', not_owned: 'auth.not_owned', no_profile: 'auth.no_profile',
  offline: 'net.request', generic: 'auth.login_failed',
};

/** Dev query params of the browser mock: ?onboarding ?mockLogin=cancel ?mockLoginError=<variant> ?mockNoKeystore ?mockRelogin ?mockLocked ?mockNoCurseforge ?mockBlockedAuto ?mockBlocks=off|preparing|error ?mockPanorama=off */
export function mockOptionsFromSearch(search: string): MockOptions {
  const q = new URLSearchParams(search);
  const err = q.get('mockLoginError');
  return {
    firstRun: q.has('onboarding'),
    login: err ? { error: err } : q.get('mockLogin') === 'cancel' ? 'cancel' : 'success',
    noKeystore: q.has('mockNoKeystore'),
    needsRelogin: q.has('mockRelogin'),
    locked: q.has('mockLocked'),
    noCurseforge: q.has('mockNoCurseforge'),
    blockedAuto: q.has('mockBlockedAuto'),
    blocks: ((['off', 'preparing', 'error'] as const).find((m) => m === q.get('mockBlocks')) ?? 'default') as MockBlocksMode,
    panorama: q.get('mockPanorama') === 'off' ? 'off' : 'default',
  };
}

const DAY = 86400000;
const uuid = () =>
  globalThis.crypto?.randomUUID?.() ??
  'xxxxxxxx-xxxx-4xxx-8xxx-xxxxxxxxxxxx'.replace(/[x]/g, () => Math.floor(Math.random() * 16).toString(16));

function seedProfiles(now: number): ProfileView[] {
  const base: Pick<ProfileView, 'coverUrl' | 'ram' | 'javaArgs' | 'mangoConfig' | 'pendingBlocked' | 'icon' | 'autoBlock' | 'background'> = { coverUrl: null, ram: null, javaArgs: '', mangoConfig: null, pendingBlocked: 0, icon: null, autoBlock: null, background: { kind: 'auto' } };
  return [
    { ...base, id: '3f1c2a7e-5b1d-4c0a-9d51-0a6f5e2b7c11', name: 'Iron Hollow', mcVersion: '1.21.4', loader: 'fabric', loaderVersion: '0.16.10',
      color: '#ffad32', ram: 6144, lastPlayed: now - 2 * 3600000, playTimeMs: 142 * 3600000 + 18 * 60000, created: now - 120 * DAY, modCount: 38 },
    { ...base, id: '8a4be9d0-2f6c-4f3e-8b0c-7d1e93a4c5f2', name: 'Vanilla Survival', mcVersion: '1.21.4', loader: 'vanilla', loaderVersion: null,
      color: '#86b86b', icon: { kind: 'block', block: 'grass_block' }, lastPlayed: now - 1 * DAY, playTimeMs: 61 * 3600000 + 5 * 60000, created: now - 300 * DAY, modCount: 0 },
    { ...base, id: 'c27d11f4-91a8-4e22-b3a6-4c8f0e9d6b37', name: 'Bedwars Practice', mcVersion: '1.8.9', loader: 'forge', loaderVersion: '11.15.1.2318',
      color: '#ed665c', icon: { kind: 'block', block: 'tnt' }, ram: 3072, javaArgs: '-XX:+UseG1GC', lastPlayed: now - 6 * DAY, playTimeMs: 23 * 3600000 + 47 * 60000, created: now - 400 * DAY, modCount: 7 },
    { ...base, id: 'e90a6b35-17c4-4d8f-a2e1-5b3c7f4d8a09', name: 'Shader Showcase', mcVersion: '1.20.6', loader: 'quilt', loaderVersion: '0.26.1',
      color: '#6fa8d6', ram: 8192, lastPlayed: now - 19 * DAY, playTimeMs: 9 * 3600000 + 12 * 60000, created: now - 60 * DAY, modCount: 21 },
    { ...base, id: '1b9d4c68-6e0f-4a35-8c27-d3f5a7e91b40', name: 'Create Expedition', mcVersion: '1.21.1', loader: 'neoforge', loaderVersion: '21.1.172',
      color: '#d6c27a', ram: 8192, lastPlayed: now - 3 * DAY, playTimeMs: 34 * 3600000 + 40 * 60000, created: now - 90 * DAY, modCount: 0 },
    { ...base, id: '6e2a85f1-0b7d-4d94-9a13-c48e2f6b7d05', name: 'Classic Forge 1.20.1', mcVersion: '1.20.1', loader: 'forge', loaderVersion: '47.3.0',
      color: '#e6a3b8', background: { kind: 'none' }, ram: 6144, lastPlayed: now - 40 * DAY, playTimeMs: 17 * 3600000 + 5 * 60000, created: now - 500 * DAY, modCount: 0 },
  ];
}

const seedAccounts = (now: number, o: MockOptions): PublicAccount[] => [
  { uuid: 'b0b7c2d4-7a41-4c3e-9f5a-12de5a3c9e01', name: 'MangoMax', type: 'microsoft', hasCape: true, headUrl: fallbackImage('account', ''),
    expiresAt: o.needsRelogin ? now - 3600000 : now + 20 * 3600000, expired: !!o.needsRelogin, needsRelogin: !!o.needsRelogin,
    credentialsPersisted: !o.noKeystore, credentialsLocked: !!o.locked },
  { uuid: '5d2e8f19-0c36-3a7b-b4d1-9e7f6a2c1b88', name: 'Gast_Steve', type: 'offline', expiresAt: null, expired: false, hasCape: false, headUrl: fallbackImage('account', ''),
    credentialsPersisted: true, credentialsLocked: false, needsRelogin: false },
];

function makeManifest(): VersionManifest {
  const versions: VersionEntry[] = [];
  const add = (id: string, type: VersionEntry['type'], t: string) => versions.push({ id, type, releaseTime: t });
  add('25w14a', 'snapshot', '2026-04-02T10:00:00Z');
  add('25w10a', 'snapshot', '2026-03-05T10:00:00Z');
  for (let p = 11; p >= 0; p--) add(p === 0 ? '1.21' : `1.21.${p}`, 'release', `2025-${String(12 - Math.min(p, 11)).padStart(2, '0')}-01T10:00:00Z`);
  add('24w40a', 'snapshot', '2024-10-02T10:00:00Z');
  for (let p = 6; p >= 0; p--) add(p === 0 ? '1.20' : `1.20.${p}`, 'release', '2024-04-01T10:00:00Z');
  for (let p = 4; p >= 0; p--) add(p === 0 ? '1.19' : `1.19.${p}`, 'release', '2023-03-01T10:00:00Z');
  for (let p = 2; p >= 0; p--) add(p === 0 ? '1.18' : `1.18.${p}`, 'release', '2022-02-01T10:00:00Z');
  for (let p = 5; p >= 0; p--) add(p === 0 ? '1.16' : `1.16.${p}`, 'release', '2021-01-01T10:00:00Z');
  for (let p = 2; p >= 0; p--) add(p === 0 ? '1.12' : `1.12.${p}`, 'release', '2017-09-01T10:00:00Z');
  add('1.8.9', 'release', '2015-12-09T10:00:00Z');
  add('1.7.10', 'release', '2014-06-26T10:00:00Z');
  add('b1.7.3', 'old_beta', '2011-07-08T10:00:00Z');
  return { latest: { release: '1.21.11', snapshot: '25w14a' }, versions, installed: ['1.21.4', '1.21.1', '1.8.9', '1.20.6'] };
}

const LOADER_VERSIONS: Record<Exclude<Loader, 'vanilla'>, LoaderVersion[]> = {
  fabric: [{ version: '0.16.10', stable: true }, { version: '0.16.9', stable: true }, { version: '0.17.0-beta.1', stable: false }],
  quilt: [{ version: '0.26.1', stable: true }, { version: '0.26.0', stable: true }],
  forge: [{ version: '54.0.12', stable: true }, { version: '54.0.3', stable: true }],
  neoforge: [{ version: '21.4.100', stable: true }, { version: '21.4.40-beta', stable: false }],
};

export type MockApi = Api & { dev: BlockedDev };

export function createMockApi(opts: MockOptions = {}): MockApi {
  const scale = opts.timeScale ?? 1;
  const now = Date.now();
  const config: Config = {
    language: 'de', ram: 4096, javaPath: '', javaArgs: '-XX:+UseG1GC', fullscreen: false, width: 1280, height: 720,
    keepLauncherOpen: true, hideOnLaunch: false, showSnapshots: false, mangoConfig: true, performanceMods: true,
    concurrentDownloads: 8, performancePreset: 'balanced', selectedProfile: null, selectedAccount: null,
    firstRunDone: !opts.firstRun, telemetry: true, sidebarOpen: true,
    blockedModsModrinthFallback: true, blockedModsWatchDirs: ['/home/mango/Downloads'], blockedModsCheckSubfolders: false,
  };
  let profiles = seedProfiles(now);
  let accounts = seedAccounts(now, opts);
  config.selectedProfile = profiles[0].id;
  config.selectedAccount = accounts[0].uuid;
  let cfOn = !opts.noCurseforge;
  const running = new Set<string>();
  const timers = new Map<string, ReturnType<typeof setTimeout>[]>();
  const listeners: { [K in keyof ApiEvents]: Set<(p: ApiEvents[K]) => void> } = {
    'launch:state': new Set(), 'launch:progress': new Set(), 'launch:log': new Set(), 'install:progress': new Set(),
    'servers:pinged': new Set(), 'update:status': new Set(), 'content:changed': new Set(), 'accounts:changed': new Set(), 'blocked:changed': new Set(),
    'blocks:status': new Set(), 'panorama:ready': new Set(),
  };
  const emit = <K extends keyof ApiEvents>(name: K, payload: ApiEvents[K]) => {
    for (const cb of listeners[name] as Set<(p: ApiEvents[K]) => void>) cb(payload);
  };
  const setState = (profileId: string, state: LaunchState) => {
    if (state === 'running') running.add(profileId);
    if (state === 'stopped' || state === 'crashed') running.delete(profileId);
    emit('launch:state', { profileId, state });
  };
  const find = (id: string) => {
    const p = profiles.find((x) => x.id === id);
    if (!p) throw { code: 'profile_not_found', message: `no profile ${id}`, needsRelogin: false };
    return p;
  };
  const clearTimers = (id: string) => {
    for (const t of timers.get(id) ?? []) clearTimeout(t);
    timers.delete(id);
  };

  const iconsMock = createIconsMock({ scale, emit, profiles: () => profiles, find, blocks: opts.blocks, panorama: opts.panorama });
  const icons = iconsMock;
  const blockedMock = createBlockedMock({
    scale, emit, auto: !!opts.blockedAuto, dirs: () => config.blockedModsWatchDirs,
    setPending: (id, n) => { const p = profiles.find((x) => x.id === id); if (p) p.pendingBlocked = n; },
  });
  const part2 = createContentApi({
    seedBlocked: blockedMock.seed,
    now, scale, emit, profiles: () => profiles, addProfile: (p) => { profiles = [...profiles, p]; icons.sync(); }, find,
    gameActive: () => running.size > 0 || timers.size > 0, get curseforge() { return cfOn; },
  });
  const api: MockApi = {
    ...part2,
    ...blockedMock.api,
    ...iconsMock.api,
    dev: blockedMock.dev,
    async appState(): Promise<AppState> {
      return {
        version: '2.0.0-dev', config: { ...config }, profiles: structuredClone(profiles), accounts: structuredClone(accounts),
        totalRamMb: 16384, running: [...running], rootDir: '~/.mangoclient',
      };
    },
    async configSet({ patch }) {
      const { curseforgeApiKey, ...rest } = patch;
      Object.assign(config, rest);
      if (curseforgeApiKey !== undefined) cfOn = !!curseforgeApiKey; // write-only, never returned
      return { ...config };
    },
    async profileCreate({ name, mcVersion, loader, loaderVersion }) {
      const p: ProfileView = {
        id: uuid(), name: name.trim(), mcVersion, loader, loaderVersion: loaderVersion ?? null, color: null, coverUrl: null, ram: null,
        javaArgs: '', mangoConfig: null, lastPlayed: null, playTimeMs: 0, created: Date.now(), modCount: 0, pendingBlocked: 0,
        icon: null, autoBlock: null, background: { kind: 'auto' },
      };
      profiles = [...profiles, p];
      icons.sync();
      return structuredClone(p);
    },
    async profileUpdate({ id, patch }) { Object.assign(find(id), patch); return structuredClone(find(id)); },
    async profileDelete({ id }) {
      profiles = profiles.filter((p) => p.id !== id);
      if (config.selectedProfile === id) config.selectedProfile = profiles[0]?.id ?? null;
    },
    async profileSelect({ id }) { find(id); config.selectedProfile = id; return { ...config }; },
    async profileDuplicate({ id }) {
      const src = find(id);
      const p = { ...structuredClone(src), id: uuid(), name: `${src.name} (2)`, lastPlayed: null, playTimeMs: 0, created: Date.now() };
      profiles = [...profiles, p];
      icons.sync();
      return structuredClone(p);
    },
    async accountAddOffline({ name }) {
      if (!/^[A-Za-z0-9_]{3,16}$/.test(name)) throw { code: 'invalid_name', message: 'invalid name', needsRelogin: false };
      const a: PublicAccount = { uuid: uuid(), name, type: 'offline', expiresAt: null, expired: false, hasCape: false, headUrl: fallbackImage('account', ''),
        credentialsPersisted: true, credentialsLocked: false, needsRelogin: false };
      accounts = [...accounts, a];
      if (!config.selectedAccount) config.selectedAccount = a.uuid;
      return { ...a };
    },
    async accountSelect({ uuid: id }) { config.selectedAccount = id; return { ...config }; },
    async accountRemove({ uuid: id }) {
      accounts = accounts.filter((a) => a.uuid !== id);
      if (config.selectedAccount === id) config.selectedAccount = accounts[0]?.uuid ?? null;
    },
    async accountSignInMicrosoft() {
      const mode = opts.login ?? 'success';
      await new Promise((r) => setTimeout(r, 1500 * scale));
      if (mode === 'cancel') return null;
      if (typeof mode === 'object') {
        throw { code: MOCK_LOGIN_ERRORS[mode.error] ?? mode.error, message: `mock login error: ${mode.error}`, needsRelogin: false };
      }
      // Signing in again revives the stale seeded account, otherwise a new fake account appears.
      const stale = accounts.find((a) => a.type === 'microsoft' && (a.needsRelogin || a.expired));
      const base = stale ?? {
        uuid: '7c1d4e90-3b52-4a8f-a6e7-2d9b0c5f8a14', name: 'CreeperKing', type: 'microsoft' as const, hasCape: false,
        headUrl: fallbackImage('account', ''), credentialsLocked: false,
      };
      const a: PublicAccount = {
        ...base, expiresAt: Date.now() + 24 * 3600000, expired: false, needsRelogin: false, credentialsLocked: false,
        credentialsPersisted: !opts.noKeystore,
      };
      accounts = accounts.some((x) => x.uuid === a.uuid) ? accounts.map((x) => (x.uuid === a.uuid ? a : x)) : [...accounts, a];
      emit('accounts:changed', { uuid: a.uuid });
      return { ...a };
    },
    async accountRefresh({ uuid: id }) {
      const a = accounts.find((x) => x.uuid === id);
      if (!a || a.type !== 'microsoft') throw { code: 'auth.invalid_account', message: 'no such account', needsRelogin: false };
      await new Promise((r) => setTimeout(r, 600 * scale));
      if (a.needsRelogin) throw { code: 'auth.needs_relogin', message: 'session expired, sign in again', needsRelogin: true };
      Object.assign(a, { expiresAt: Date.now() + 24 * 3600000, expired: false, credentialsLocked: false });
      emit('accounts:changed', { uuid: id });
      return { ...a };
    },
    async versionsManifest() { return makeManifest(); },
    async loaderVersions({ loader }) { return loader === 'vanilla' ? [] : structuredClone(LOADER_VERSIONS[loader]); },
    async gameLaunch({ profileId }) {
      const p = find(profileId);
      if (running.has(profileId) || timers.has(profileId)) throw { code: 'already_running', message: 'already running', needsRelogin: false };
      const acc = accounts.find((a) => a.uuid === config.selectedAccount);
      if (acc?.type === 'microsoft' && (acc.needsRelogin || acc.expired)) {
        setState(profileId, 'preparing');
        await new Promise((r) => setTimeout(r, 500 * scale));
        setState(profileId, 'stopped');
        throw { code: 'auth.needs_relogin', message: 'session expired, sign in again', needsRelogin: true };
      }
      const list: ReturnType<typeof setTimeout>[] = [];
      timers.set(profileId, list);
      const at = (ms: number, fn: () => void) => list.push(setTimeout(fn, ms * scale));
      const log = (line: string, level: 'info' | 'warn' | 'error' = 'info') => emit('launch:log', { profileId, line, level });
      const prog = (phase: ApiEvents['launch:progress']['phase'], done: number, total: number, label: string) =>
        emit('launch:progress', { profileId, phase, done, total, label });

      return new Promise((resolve) => {
        at(0, () => { setState(profileId, 'preparing'); log(`[launcher] Preparing ${p.name} (${p.mcVersion}, ${p.loader})`); });
        at(400, () => { setState(profileId, 'java'); prog('java', 1, 1, 'temurin-21'); log('[launcher] Using Java 21 (Temurin)'); });
        at(900, () => { setState(profileId, 'loader'); log(`[launcher] Loader ${p.loader} ${p.loaderVersion ?? ''}`.trim()); });
        at(1300, () => setState(profileId, 'downloading'));
        const steps: ['client' | 'libraries' | 'assets', number, string][] = [['client', 1, 'client.jar'], ['libraries', 84, 'libraries'], ['assets', 3400, 'assets']];
        steps.forEach(([phase, total, label], i) => {
          for (let k = 1; k <= 5; k++) at(1300 + i * 1000 + k * 190, () => prog(phase, Math.round((total * k) / 5), total, label));
        });
        at(1300 + 3 * 1000 + 100, () => prog('done', 1, 1, ''));
        at(4400, () => { setState(profileId, 'account'); log('[launcher] Account session valid'); });
        at(4900, () => setState(profileId, 'launching'));
        at(5000, () => resolve({ pid: 41872, versionId: p.mcVersion }));
        at(5200, () => { setState(profileId, 'running'); p.lastPlayed = Date.now(); });
        at(5300, () => log('[main/INFO]: Loading Minecraft ' + p.mcVersion));
        at(5500, () => log('[main/INFO]: Loading ' + p.modCount + ' mods'));
        at(5800, () => log('[main/WARN]: Config file sodium-options.json was outdated', 'warn'));
        at(6100, () => log('[Render thread/INFO]: Backend initialized'));
      });
    },
    async gameStop({ profileId }) {
      const known = running.has(profileId) || timers.has(profileId);
      if (!known) return false;
      clearTimers(profileId);
      const list: ReturnType<typeof setTimeout>[] = [];
      list.push(setTimeout(() => { setState(profileId, 'stopped'); timers.delete(profileId); }, 300 * scale));
      timers.set(profileId, list);
      return true;
    },
    async gameRunning() { return [...running]; },
    async openFolder({ target }) { return `~/.mangoclient/${target}`; },
    async openExternal({ url }) {
      if (typeof window !== 'undefined') window.open(url, '_blank', 'noopener,noreferrer');
    },
    async listen<K extends keyof ApiEvents>(name: K, cb: (p: ApiEvents[K]) => void): Promise<Unlisten> {
      const set = listeners[name] as Set<(p: ApiEvents[K]) => void>;
      set.add(cb);
      return () => { set.delete(cb); };
    },
  };
  return api;
}
