import type {
  Api, Unlisten,
} from './api';
import { createContentApi } from './mockContent';
import { fallbackImage } from './fallbackImage';
import type {
  ApiEvents, AppState, Config, LaunchState, LoaderVersion, ProfileView, PublicAccount, VersionManifest, VersionEntry, Loader,
} from './types';

export interface MockOptions {
  /** Multiplies every simulated delay. 1 = about 6 s per launch. */
  timeScale?: number;
  /** Start with config.firstRunDone=false so the onboarding shows. */
  firstRun?: boolean;
}

const DAY = 86400000;
const uuid = () =>
  globalThis.crypto?.randomUUID?.() ??
  'xxxxxxxx-xxxx-4xxx-8xxx-xxxxxxxxxxxx'.replace(/[x]/g, () => Math.floor(Math.random() * 16).toString(16));

function seedProfiles(now: number): ProfileView[] {
  const base = { coverUrl: null, ram: null, javaArgs: '', mangoConfig: null };
  return [
    { ...base, id: '3f1c2a7e-5b1d-4c0a-9d51-0a6f5e2b7c11', name: 'Iron Hollow', mcVersion: '1.21.4', loader: 'fabric', loaderVersion: '0.16.10',
      color: '#ffad32', ram: 6144, lastPlayed: now - 2 * 3600000, playTimeMs: 142 * 3600000 + 18 * 60000, created: now - 120 * DAY, modCount: 38 },
    { ...base, id: '8a4be9d0-2f6c-4f3e-8b0c-7d1e93a4c5f2', name: 'Vanilla Survival', mcVersion: '1.21.1', loader: 'vanilla', loaderVersion: null,
      color: '#86b86b', lastPlayed: now - 1 * DAY, playTimeMs: 61 * 3600000 + 5 * 60000, created: now - 300 * DAY, modCount: 0 },
    { ...base, id: 'c27d11f4-91a8-4e22-b3a6-4c8f0e9d6b37', name: 'Bedwars Practice', mcVersion: '1.8.9', loader: 'forge', loaderVersion: '11.15.1.2318',
      color: '#ed665c', ram: 3072, javaArgs: '-XX:+UseG1GC', lastPlayed: now - 6 * DAY, playTimeMs: 23 * 3600000 + 47 * 60000, created: now - 400 * DAY, modCount: 7 },
    { ...base, id: 'e90a6b35-17c4-4d8f-a2e1-5b3c7f4d8a09', name: 'Shader Showcase', mcVersion: '1.20.6', loader: 'quilt', loaderVersion: '0.26.1',
      color: '#6fa8d6', ram: 8192, lastPlayed: now - 19 * DAY, playTimeMs: 9 * 3600000 + 12 * 60000, created: now - 60 * DAY, modCount: 21 },
  ];
}

const seedAccounts = (now: number): PublicAccount[] => [
  { uuid: 'b0b7c2d4-7a41-4c3e-9f5a-12de5a3c9e01', name: 'MangoMax', type: 'microsoft', expiresAt: now + 20 * 3600000, expired: false, hasCape: true, headUrl: fallbackImage('account', 'b0b7') },
  { uuid: '5d2e8f19-0c36-3a7b-b4d1-9e7f6a2c1b88', name: 'Gast_Steve', type: 'offline', expiresAt: null, expired: false, hasCape: false, headUrl: fallbackImage('account', '5d2e') },
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

export function createMockApi(opts: MockOptions = {}): Api {
  const scale = opts.timeScale ?? 1;
  const now = Date.now();
  const config: Config = {
    language: 'de', ram: 4096, javaPath: '', javaArgs: '-XX:+UseG1GC', fullscreen: false, width: 1280, height: 720,
    keepLauncherOpen: true, hideOnLaunch: false, showSnapshots: false, mangoConfig: true, performanceMods: true,
    concurrentDownloads: 8, performancePreset: 'balanced', selectedProfile: null, selectedAccount: null,
    firstRunDone: !opts.firstRun, telemetry: true, sidebarOpen: true,
  };
  let profiles = seedProfiles(now);
  let accounts = seedAccounts(now);
  config.selectedProfile = profiles[0].id;
  config.selectedAccount = accounts[0].uuid;
  const running = new Set<string>();
  const timers = new Map<string, ReturnType<typeof setTimeout>[]>();
  const listeners: { [K in keyof ApiEvents]: Set<(p: ApiEvents[K]) => void> } = {
    'launch:state': new Set(), 'launch:progress': new Set(), 'launch:log': new Set(), 'install:progress': new Set(),
    'servers:pinged': new Set(), 'update:status': new Set(), 'content:changed': new Set(),
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

  const part2 = createContentApi({
    now, scale, emit, profiles: () => profiles, addProfile: (p) => { profiles = [...profiles, p]; }, find,
    gameActive: () => running.size > 0 || timers.size > 0,
  });
  const api: Api = {
    ...part2,
    async appState(): Promise<AppState> {
      return {
        version: '2.0.0-dev', config: { ...config }, profiles: structuredClone(profiles), accounts: structuredClone(accounts),
        totalRamMb: 16384, running: [...running], rootDir: '~/.mangoclient',
      };
    },
    async configSet({ patch }) { Object.assign(config, patch); return { ...config }; },
    async profileCreate({ name, mcVersion, loader, loaderVersion }) {
      const p: ProfileView = {
        id: uuid(), name: name.trim(), mcVersion, loader, loaderVersion: loaderVersion ?? null, color: null, coverUrl: null, ram: null,
        javaArgs: '', mangoConfig: null, lastPlayed: null, playTimeMs: 0, created: Date.now(), modCount: 0,
      };
      profiles = [...profiles, p];
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
      return structuredClone(p);
    },
    async accountAddOffline({ name }) {
      if (!/^[A-Za-z0-9_]{3,16}$/.test(name)) throw { code: 'invalid_name', message: 'invalid name', needsRelogin: false };
      const a: PublicAccount = { uuid: uuid(), name, type: 'offline', expiresAt: null, expired: false, hasCape: false, headUrl: fallbackImage('account', name) };
      accounts = [...accounts, a];
      if (!config.selectedAccount) config.selectedAccount = a.uuid;
      return { ...a };
    },
    async accountSelect({ uuid: id }) { config.selectedAccount = id; return { ...config }; },
    async accountRemove({ uuid: id }) {
      accounts = accounts.filter((a) => a.uuid !== id);
      if (config.selectedAccount === id) config.selectedAccount = accounts[0]?.uuid ?? null;
    },
    async accountSignInMicrosoft() { throw { code: 'not_implemented', message: 'not implemented', needsRelogin: false }; },
    async versionsManifest() { return makeManifest(); },
    async loaderVersions({ loader }) { return loader === 'vanilla' ? [] : structuredClone(LOADER_VERSIONS[loader]); },
    async gameLaunch({ profileId }) {
      const p = find(profileId);
      if (running.has(profileId) || timers.has(profileId)) throw { code: 'already_running', message: 'already running', needsRelogin: false };
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
    async listen<K extends keyof ApiEvents>(name: K, cb: (p: ApiEvents[K]) => void): Promise<Unlisten> {
      const set = listeners[name] as Set<(p: ApiEvents[K]) => void>;
      set.add(cb);
      return () => { set.delete(cb); };
    },
  };
  return api;
}
