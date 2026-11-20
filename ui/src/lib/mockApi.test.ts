import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createMockApi } from './mockApi';
import type { ApiEvents } from './types';

describe('mockApi', () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it('serves seed data', async () => {
    const api = createMockApi();
    const s = await api.appState();
    expect(s.profiles).toHaveLength(6);
    expect(s.accounts).toHaveLength(2);
    expect(s.profiles.some((p) => p.name === 'Iron Hollow' && p.mcVersion === '1.21.4' && p.loader === 'fabric')).toBe(true);
  });

  it('emits the launch state sequence in order and ends running', async () => {
    const api = createMockApi();
    const id = (await api.appState()).profiles[0].id;
    const states: string[] = [];
    const phases: string[] = [];
    await api.listen('launch:state', (e) => states.push(e.state));
    await api.listen('launch:progress', (e) => phases.push(e.phase));
    const p = api.gameLaunch({ profileId: id });
    await vi.advanceTimersByTimeAsync(6500);
    expect((await p).versionId).toBe('1.21.4');
    expect(states).toEqual(['preparing', 'java', 'loader', 'downloading', 'account', 'launching', 'running']);
    expect(phases).toContain('assets');
    expect(await api.gameRunning()).toEqual([id]);
  });

  it('streams logs and stop ends with stopped', async () => {
    const api = createMockApi();
    const id = (await api.appState()).profiles[0].id;
    const logs: ApiEvents['launch:log'][] = [];
    const states: string[] = [];
    await api.listen('launch:log', (e) => logs.push(e));
    await api.listen('launch:state', (e) => states.push(e.state));
    void api.gameLaunch({ profileId: id });
    await vi.advanceTimersByTimeAsync(6500);
    expect(logs.length).toBeGreaterThan(3);
    expect(logs.some((l) => l.level === 'warn')).toBe(true);
    expect(await api.gameStop({ profileId: id })).toBe(true);
    await vi.advanceTimersByTimeAsync(500);
    expect(states.at(-1)).toBe('stopped');
    expect(await api.gameRunning()).toEqual([]);
  });

  it('creates profiles and rejects bad offline names with a code', async () => {
    const api = createMockApi();
    const p = await api.profileCreate({ name: ' Test ', mcVersion: '1.21.4', loader: 'vanilla' });
    expect(p.name).toBe('Test');
    await expect(api.accountAddOffline({ name: 'x' })).rejects.toMatchObject({ code: 'invalid_name' });
    const login = api.accountSignInMicrosoft();
    await vi.advanceTimersByTimeAsync(2000);
    expect(await login).toMatchObject({ type: 'microsoft', needsRelogin: false, credentialsPersisted: true });
  });

  it('config_set applies the patch and onboarding starts unfinished with firstRun', async () => {
    const api = createMockApi({ firstRun: true });
    expect((await api.appState()).config.firstRunDone).toBe(false);
    const c = await api.configSet({ patch: { language: 'en', firstRunDone: true, ram: 6144 } });
    expect(c).toMatchObject({ language: 'en', firstRunDone: true, ram: 6144 });
    expect((await api.appState()).config.language).toBe('en');
  });
});

describe('mock icons and backgrounds (Parts 6 + 7)', () => {
  it('prepares the gallery, fills autoBlock, then lets a block be chosen', async () => {
    vi.useFakeTimers();
    const api = createMockApi();
    const events: string[] = [];
    await api.listen('blocks:status', (s) => events.push(s.state));
    expect((await api.blockIconsStatus()).state).toBe('missing');
    expect((await api.appState()).profiles[0].autoBlock).toBeNull();
    expect((await api.blockIconsPrepare()).state).toBe('preparing');
    await vi.advanceTimersByTimeAsync(2500);
    expect(events).toEqual(['ready']);
    const st = await api.blockIconsStatus();
    expect(st.blocks).toHaveLength(21);
    const ps = (await api.appState()).profiles;
    expect(ps.every((p) => p.autoBlock && st.blocks.some((b) => b.id === p.autoBlock))).toBe(true);
    const v = await api.profileIconSetBlock({ id: ps[0].id, block: 'tnt' });
    expect(v.icon).toEqual({ kind: 'block', block: 'tnt' });
    await expect(api.profileIconSetBlock({ id: ps[0].id, block: 'bedrock' })).rejects.toMatchObject({ code: 'block_unknown' });
    expect((await api.profileIconClear({ id: ps[0].id })).icon).toBeNull();
    vi.useRealTimers();
  });
  it('mockBlocks switch: off stays missing, error ends in error', async () => {
    vi.useFakeTimers();
    const off = createMockApi({ blocks: 'off' });
    expect((await off.blockIconsPrepare()).state).toBe('missing');
    const bad = createMockApi({ blocks: 'error' });
    expect((await bad.blockIconsStatus()).errorCode).toBe('blocks.jar_unreadable');
    expect((await createMockApi({ blocks: 'preparing' }).blockIconsStatus()).state).toBe('preparing');
    vi.useRealTimers();
  });
  it('backgrounds: set, clear, none; panorama null first then ready event', async () => {
    vi.useFakeTimers();
    const api = createMockApi();
    const id = (await api.appState()).profiles[0].id;
    const ready: string[] = [];
    await api.listen('panorama:ready', (e) => ready.push(e.profileId));
    expect(await api.panoramaForProfile({ id })).toBeNull();
    await vi.advanceTimersByTimeAsync(1200);
    expect(ready).toEqual([id]);
    expect((await api.panoramaForProfile({ id }))?.faces).toHaveLength(6);
    expect((await api.profileBackgroundSetImage({ id, filePath: 'blob:abc' })).background).toEqual({ kind: 'image', url: 'blob:abc' });
    expect((await api.profileBackgroundClear({ id, none: true })).background).toEqual({ kind: 'none' });
    expect((await api.profileBackgroundClear({ id })).background).toEqual({ kind: 'auto' });
    await expect(api.profileIconSetImage({ id, filePath: 'x.exe' })).rejects.toMatchObject({ code: 'image_unsupported_type' });
    vi.useRealTimers();
  });
});
