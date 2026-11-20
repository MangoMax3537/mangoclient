import { derivedRam } from './ram';
import type {
  Api, Unlisten,
} from './api';
import { createContentApi } from './mockContent';
import { mockDevFromSearch, withDevControls, type MockDevOptions } from './mockDev';
import { createIconsMock, type MockBlocksMode, type MockPanoramaMode } from './mockIcons';
import { createWorldsMock } from './mockWorlds';
import { createSkinsMock } from './mockSkins';
import { createPortabilityMock, MOCK_CRASHES, type MockCrash, type PortabilityDev } from './mockPortability';
import { checkGroup, checkTag } from './portability';
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
  /** Part 6: how the block gallery behaves (see mockIcons). */
  blocks?: MockBlocksMode;
  /** Part 7: `off` = no panorama is ever extracted. */
  panorama?: MockPanoramaMode;
  /** Part 9: the first instance's game is running from the start (`?mockRunning`): deleting worlds and restoring fail with game_running. */
  running?: boolean;
  /** Part 9: every changing world/backup call fails with this code (`?mockBackupError=<code>`). */
  backupError?: string;
  /** Part 9: which crash `crash_analyze` reports (`?mockCrash=java|oom|dependency|none`); any value also makes a launch end in a crash. */
  crash?: MockCrash;
  /** Part 9: every import plan / import fails with this code (`?mockImportError=<code>`), exports with `exportError`. */
  importError?: string;
  exportError?: string;
  /** Part 9: simulate `files:dropped` shortly after the app listens (`?mockDrop=mrpack|curseforge|prism|bad`; a CurseForge zip is rejected with import.unsupported_curseforge). */
  drop?: string;
  /** Part 13: `?mockSkinError=<code>` makes uploads, username copies and cape changes fail; `?mockCapes=0` = an account without capes. */
  skinError?: string;
  noCapes?: boolean;
  /** Latency / slow / fail controls (`?mockLatency`, `?mockDelay`, `?mockSlow`, `?mockFail`). Absent = no wrapping (unit tests). */
  dev?: MockDevOptions;
}

/** Error code per `?mockLoginError=<variant>` (codes from crates/auth/src/error.rs). */
export const MOCK_LOGIN_ERRORS: Record<string, string> = {
  no_xbox_profile: 'auth.xbox.no_profile', country: 'auth.xbox.country', adult_verification: 'auth.xbox.adult_verification',
  child_account: 'auth.xbox.child_account', not_owned: 'auth.not_owned', no_profile: 'auth.no_profile',
  offline: 'net.request', generic: 'auth.login_failed',
};

/** Dev query params of the browser mock: ?onboarding ?mockLogin=cancel ?mockLoginError=<variant> ?mockNoKeystore ?mockRelogin ?mockLocked ?mockBlocks=off|preparing|error ?mockPanorama=off ?mockRunning ?mockBackupError=<code> ?mockCrash=java|oom|dependency|none ?mockImportError=<code> ?mockExportError=<code> ?mockDrop=mrpack|curseforge|prism|bad */
export function mockOptionsFromSearch(search: string): MockOptions {
  const q = new URLSearchParams(search);
  const err = q.get('mockLoginError');
  return {
    firstRun: q.has('onboarding'),
    login: err ? { error: err } : q.get('mockLogin') === 'cancel' ? 'cancel' : 'success',
    noKeystore: q.has('mockNoKeystore'),
    needsRelogin: q.has('mockRelogin'),
    locked: q.has('mockLocked'),
    blocks: ((['off', 'preparing', 'error'] as const).find((m) => m === q.get('mockBlocks')) ?? 'default') as MockBlocksMode,
    panorama: q.get('mockPanorama') === 'off' ? 'off' : 'default',
    running: q.has('mockRunning'),
    backupError: q.get('mockBackupError') || undefined,
    crash: MOCK_CRASHES.find((m) => m === q.get('mockCrash')),
    importError: q.get('mockImportError') || undefined,
    exportError: q.get('mockExportError') || undefined,
    drop: q.has('mockDrop') ? q.get('mockDrop') || 'mrpack' : undefined,
    skinError: q.get('mockSkinError') || undefined,
    noCapes: q.get('mockCapes') === '0',
    dev: mockDevFromSearch(search),
  };
}

const DAY = 86400000;
const nope = (): Promise<never> => Promise.reject({ code: 'not_implemented', message: 'not available in the browser mock', needsRelogin: false });
const uuid = () =>
  globalThis.crypto?.randomUUID?.() ??
  'xxxxxxxx-xxxx-4xxx-8xxx-xxxxxxxxxxxx'.replace(/[x]/g, () => Math.floor(Math.random() * 16).toString(16));

function seedProfiles(now: number): ProfileView[] {
  const base: Pick<ProfileView, 'coverUrl' | 'ram' | 'ramSource' | 'ramHint' | 'effectiveRamMb' | 'javaArgs' | 'mangoConfig' | 'javaPath' | 'window' | 'icon' | 'iconSource' | 'autoBlock' | 'background' | 'group' | 'tags'> = { coverUrl: null, ram: null, ramSource: 'default', ramHint: null, effectiveRamMb: 4096, javaArgs: '', javaPath: null, window: { mode: 'default', width: null, height: null }, mangoConfig: null, icon: null, iconSource: null, autoBlock: null, background: { kind: 'auto' }, group: null, tags: [] };
  return [
    { ...base, id: '3f1c2a7e-5b1d-4c0a-9d51-0a6f5e2b7c11', name: 'Iron Hollow', mcVersion: '1.21.4', loader: 'fabric', loaderVersion: '0.16.10',
      color: '#ffad32', ram: 6144, ramSource: 'user', effectiveRamMb: 6144, group: 'Survival', tags: ['main', 'fabric'], lastPlayed: now - 2 * 3600000, playTimeMs: 142 * 3600000 + 18 * 60000, created: now - 120 * DAY, modCount: 38 },
    { ...base, id: '8a4be9d0-2f6c-4f3e-8b0c-7d1e93a4c5f2', name: 'Vanilla Survival', mcVersion: '1.21.4', loader: 'vanilla', loaderVersion: null,
      color: '#86b86b', icon: { kind: 'block', block: 'grass_block' }, group: 'Survival', tags: ['vanilla', 'friends'], lastPlayed: now - 1 * DAY, playTimeMs: 61 * 3600000 + 5 * 60000, created: now - 300 * DAY, modCount: 0 },
    { ...base, id: 'c27d11f4-91a8-4e22-b3a6-4c8f0e9d6b37', name: 'Bedwars Practice', mcVersion: '1.8.9', loader: 'forge', loaderVersion: '11.15.1.2318',
      color: '#ed665c', icon: { kind: 'block', block: 'tnt' }, group: 'PvP', tags: ['1.8', 'practice'], ram: 3072, ramSource: 'user', effectiveRamMb: 3072, javaArgs: '-XX:+UseG1GC', lastPlayed: now - 6 * DAY, playTimeMs: 23 * 3600000 + 47 * 60000, created: now - 400 * DAY, modCount: 7 },
    { ...base, id: 'e90a6b35-17c4-4d8f-a2e1-5b3c7f4d8a09', name: 'Shader Showcase', mcVersion: '1.20.6', loader: 'quilt', loaderVersion: '0.26.1',
      color: '#6fa8d6', ram: 8192, ramSource: 'user', effectiveRamMb: 8192, tags: ['shaders'], lastPlayed: now - 19 * DAY, playTimeMs: 9 * 3600000 + 12 * 60000, created: now - 60 * DAY, modCount: 21 },
    { ...base, id: '1b9d4c68-6e0f-4a35-8c27-d3f5a7e91b40', name: 'Create Expedition', mcVersion: '1.21.1', loader: 'neoforge', loaderVersion: '21.1.172',
      color: '#d6c27a', ram: 4096, ramSource: 'modpack', effectiveRamMb: 4096, ramHint: { recommendedMb: 4096, minMb: 3072, maxMb: 6144, source: 'modrinth' }, group: 'Modpacks', tags: ['create', 'tech'], lastPlayed: now - 3 * DAY, playTimeMs: 34 * 3600000 + 40 * 60000, created: now - 90 * DAY, modCount: 0 },
    { ...base, id: '6e2a85f1-0b7d-4d94-9a13-c48e2f6b7d05', name: 'Classic Forge 1.20.1', mcVersion: '1.20.1', loader: 'forge', loaderVersion: '47.3.0',
      color: '#e6a3b8', background: { kind: 'none' }, group: 'Modpacks', ram: 6144, ramSource: 'user', effectiveRamMb: 6144, lastPlayed: now - 40 * DAY, playTimeMs: 17 * 3600000 + 5 * 60000, created: now - 500 * DAY, modCount: 0 },
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

export type MockApi = Api & { dev: PortabilityDev };

export function createMockApi(opts: MockOptions = {}): MockApi {
  const scale = opts.timeScale ?? 1;
  const now = Date.now();
  const config: Config = {
    language: 'de', ram: 4096, javaPath: '', javaArgs: '-XX:+UseG1GC', fullscreen: false, width: 1280, height: 720,
    keepLauncherOpen: true, hideOnLaunch: false, showSnapshots: false, mangoConfig: true, performanceMods: true,
    concurrentDownloads: 8, performancePreset: 'balanced', selectedProfile: null, selectedAccount: null,
    firstRunDone: !opts.firstRun, telemetry: true, sidebarOpen: true,
    autoSnapshot: true, backupKeep: 5,
  };
  let profiles = seedProfiles(now);
  let accounts = seedAccounts(now, opts);
  setTimeout(() => skinsMock.init(), 0);
  config.selectedProfile = profiles[0].id;
  config.selectedAccount = accounts[0].uuid;
  const running = new Set<string>(opts.running ? [profiles[0].id] : []);
  const timers = new Map<string, ReturnType<typeof setTimeout>[]>();
  const listeners: { [K in keyof ApiEvents]: Set<(p: ApiEvents[K]) => void> } = {
    'launch:state': new Set(), 'launch:progress': new Set(), 'launch:log': new Set(), 'install:progress': new Set(),
    'servers:pinged': new Set(), 'update:status': new Set(), 'content:changed': new Set(), 'accounts:changed': new Set(),
    'blocks:status': new Set(), 'panorama:ready': new Set(), 'files:dropped': new Set(), 'skin:updated': new Set(), 'java:progress': new Set(), 'window:state': new Set(),
  };
  const emitDrop = (paths: string[]) => emit('files:dropped', { paths });
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
  const worldsMock = createWorldsMock({
    scale, now, config: () => config, profiles: () => profiles, find, errorCode: opts.backupError,
    gameActive: (id) => running.has(id) || timers.has(id),
  });
  const portabilityMock = createPortabilityMock({
    scale, emit, emitDrop, profiles: () => profiles, find, addProfile: (p) => { profiles = [...profiles, p]; icons.sync(); },
    crash: opts.crash, importError: opts.importError, exportError: opts.exportError,
  });
  const skinsMock = createSkinsMock({
    scale, emit, accounts: () => accounts, error: opts.skinError, noCapes: opts.noCapes,
    setHead: (id, url) => { accounts = accounts.map((a) => (a.uuid === id ? { ...a, headUrl: url } : a)); },
  });
  const javaMock = {
    list: [
      { path: '/usr/lib/jvm/java-21-openjdk/bin/java', major: 21, raw: 'openjdk version "21.0.5" 2024-10-15', vendor: 'OpenJDK', source: 'system' as const },
      { path: '~/.mangoclient/runtimes/temurin-17/bin/java', major: 17, raw: 'openjdk version "17.0.13" 2024-10-15', vendor: 'Temurin', source: 'managed' as const },
    ],
  };
  const part2 = createContentApi({
    now, scale, emit, profiles: () => profiles, addProfile: (p) => { profiles = [...profiles, p]; icons.sync(); }, find,
    gameActive: () => running.size > 0 || timers.size > 0,
  });
  const api: MockApi = {
    ...part2,
    ...iconsMock.api,
    ...skinsMock.api,
    ...worldsMock,
    ...portabilityMock.api,
    dev: { ...portabilityMock.dev },
    async appState(): Promise<AppState> {
      return {
        version: '2.0.0-dev', config: { ...config }, profiles: structuredClone(profiles).map((p) => ({ ...p, ...derivedRam(p.ram, p.ramSource, config.ram) })), accounts: structuredClone(accounts),
        totalRamMb: 16384, defaultRamMb: 4096, ramLimits: { minMb: 1024, maxMb: 14336, totalMb: 16384 }, running: [...running], rootDir: '~/.mangoclient',
      };
    },
    async configSet({ patch }) {
      // like the Rust dispatch: unknown keys are rejected, not ignored
      const unknown = Object.keys(patch).find((k) => !(k in config));
      if (unknown) throw { code: 'validation', message: `Unknown configuration setting: ${unknown}`, needsRelogin: false };
      if (patch.backupKeep !== undefined && !(Number.isInteger(patch.backupKeep) && patch.backupKeep >= 1 && patch.backupKeep <= 50)) {
        throw { code: 'validation', message: 'backupKeep must be 1..50', needsRelogin: false };
      }
      Object.assign(config, patch);
      return { ...config };
    },
    async profileCreate({ name, mcVersion, loader, loaderVersion }) {
      const p: ProfileView = {
        id: uuid(), name: name.trim(), mcVersion, loader, loaderVersion: loaderVersion ?? null, color: null, coverUrl: null, ram: null, ramSource: 'default', ramHint: null, effectiveRamMb: config.ram,
        javaArgs: '', javaPath: null, window: { mode: 'default', width: null, height: null }, mangoConfig: null, lastPlayed: null, playTimeMs: 0, created: Date.now(), modCount: 0,
        icon: null, iconSource: null, autoBlock: null, background: { kind: 'auto' }, group: null, tags: [],
      };
      profiles = [...profiles, p];
      icons.sync();
      return structuredClone(p);
    },
    async profileUpdate({ id, patch }) {
      const p = find(id);
      const next: Partial<ProfileView> = { ...patch };
      // like the service: group is trimmed (blank = none, 48 chars), tags are trimmed, deduplicated case-insensitively (12 of 24 chars)
      if ('group' in patch) {
        const g = checkGroup(patch.group ?? '');
        if (!g.ok) throw { code: 'validation', message: `group: ${g.error}`, needsRelogin: false };
        next.group = g.group;
      }
      if (patch.tags) {
        const out: string[] = [];
        for (const raw of patch.tags) {
          const r = checkTag(raw, out);
          if (r.ok) out.push(r.tag);
          else if (r.error === 'long' || r.error === 'control' || r.error === 'limit') throw { code: 'validation', message: `tag: ${r.error}`, needsRelogin: false };
        }
        next.tags = out;
      }
      if ('javaPath' in patch) {
        const jp = (patch.javaPath ?? '').trim();
        if (jp && !(jp.startsWith('/') || /^[A-Za-z]:\\/.test(jp))) throw { code: 'java.not_runnable', message: 'not an absolute path', needsRelogin: false };
        if (jp && !javaMock.list.some((j) => j.path === jp) && !/\bjava(\.exe)?$/i.test(jp)) throw { code: 'java.not_runnable', message: 'does not run', needsRelogin: false };
        next.javaPath = jp || null;
      }
      if (patch.window) {
        const w = patch.window;
        if (w.mode === 'size' && !(Number.isInteger(w.width) && Number.isInteger(w.height) && (w.width ?? 0) >= 640 && (w.width ?? 0) <= 16384 && (w.height ?? 0) >= 480 && (w.height ?? 0) <= 16384)) {
          throw { code: 'validation', message: 'window size out of range', needsRelogin: false };
        }
        next.window = w.mode === 'size' ? w : { mode: w.mode, width: null, height: null };
      }
      if ('ram' in patch) {
        const r = patch.ram ?? null;
        if (r !== null && !(Number.isInteger(r) && r >= 1024 && r <= 14336)) throw { code: 'validation', message: 'ram out of range', needsRelogin: false };
        const hit = p.ramHint?.recommendedMb ?? null;
        next.ramSource = r === null ? 'default' : hit !== null && r === hit ? 'modpack' : 'user';
      }
      Object.assign(p, next);
      Object.assign(p, derivedRam(p.ram, p.ramSource, config.ram));
      return structuredClone(p);
    },
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
        if (opts.crash) {
          at(7600, () => {
            log('[Render thread/ERROR]: Unhandled game exception', 'error');
            clearTimers(profileId); setState(profileId, 'crashed');
          });
        }
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
    // Commands of features the browser mock does not simulate (skins, Java installs, the frameless window): they answer like the service
    // does before the feature exists, so the UI never hangs on them in a plain browser.
    async javaList() { return structuredClone(javaMock.list); },
    async javaInstall({ major }) {
      const total = 100;
      for (let i = 1; i <= 5; i++) { await new Promise((r) => setTimeout(r, 350 * scale)); emit('java:progress', { major, done: (total * i) / 5, total }); }
      const j = { path: `~/.mangoclient/runtimes/temurin-${major}/bin/java`, major, raw: `openjdk version "${major}.0.1"`, vendor: 'Temurin', source: 'managed' as const };
      javaMock.list = [...javaMock.list.filter((x) => x.major !== major), j];
      return structuredClone(j);
    },
    async panoramaPool() { return icons.pool(); },
    async windowMinimize() {}, async windowToggleMaximize() {}, async windowClose() {}, async windowIsMaximized() { return false; },
    async windowStartDrag() {}, async windowStartResize() {},
    async pickImageFile() { return null; }, // no native dialog in the browser: the UI uses <input type=file> there
    async pickFolder() { return null; },
    async openFolder({ target }) { return `~/.mangoclient/${target}`; },
    async openExternal({ url }) {
      if (typeof window !== 'undefined') window.open(url, '_blank', 'noopener,noreferrer');
    },
    async listen<K extends keyof ApiEvents>(name: K, cb: (p: ApiEvents[K]) => void): Promise<Unlisten> {
      if (name === 'files:dropped') {
        const set = listeners['files:dropped'];
        set.add(cb as (p: ApiEvents['files:dropped']) => void);
        if (opts.drop && set.size === 1) {
          const kind = opts.drop;
          setTimeout(() => emitDrop([kind === 'bad' ? '/home/mango/Downloads/notes.txt' : kind === 'curseforge' ? portabilityMock.dev.samples[1] : kind === 'prism' ? portabilityMock.dev.samples[2] : portabilityMock.dev.samples[0]]), 900 * scale);
        }
        return () => { set.delete(cb as (p: ApiEvents['files:dropped']) => void); };
      }
      const set = listeners[name] as Set<(p: ApiEvents[K]) => void>;
      set.add(cb);
      return () => { set.delete(cb); };
    },
  };
  return opts.dev ? withDevControls(api, opts.dev) : api;
}
