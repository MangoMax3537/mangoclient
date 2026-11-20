import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createMockApi } from './mockApi';
import type { ApiEvents } from './types';

describe('mockApi', () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it('serves seed data', async () => {
    const api = createMockApi();
    const s = await api.appState();
    expect(s.profiles).toHaveLength(4);
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
