import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createMockApi } from './mockApi';
import {
  actions, anyGameActive, app, applyInstallProgress, applyPinged, applyProgress, applyState, CONSOLE_CAP, consoles, init, installs, isBusy,
  launchInfo, nav, openDiscover, openProfile, pushLog, resetStores, servers, stageProgress, t, toast, toasts, updates,
} from './stores.svelte';

describe('stores', () => {
  beforeEach(() => { vi.useFakeTimers(); resetStores(); });
  afterEach(() => vi.useRealTimers());

  it('caps the console buffer', () => {
    for (let i = 0; i < CONSOLE_CAP + 50; i++) pushLog('p', `line ${i}`, 'info');
    expect(consoles['p']).toHaveLength(CONSOLE_CAP);
    expect(consoles['p'][0].line).toBe('line 50');
  });

  it('derives busy/progress from events', () => {
    expect(isBusy('p')).toBe(false);
    applyState('p', 'downloading');
    expect(isBusy('p')).toBe(true);
    applyProgress({ profileId: 'p', phase: 'libraries', done: 42, total: 84, label: '' });
    expect(launchInfo('p').downloadFraction).toBeCloseTo(0.5, 5);
    const mid = stageProgress(launchInfo('p'));
    applyState('p', 'running');
    expect(stageProgress(launchInfo('p'))).toBe(1);
    expect(mid).toBeGreaterThan(0.4);
    applyState('p', 'stopped');
    expect(isBusy('p')).toBe(false);
  });

  it('loads app state from the api and follows a launch', async () => {
    const api = createMockApi();
    await init(api);
    expect(app.ready).toBe(true);
    expect(app.profiles).toHaveLength(6);
    const id = app.profiles[0].id;
    void api.gameLaunch({ profileId: id });
    await vi.advanceTimersByTimeAsync(6500);
    expect(launchInfo(id).state).toBe('running');
    expect(consoles[id].length).toBeGreaterThan(0);
    expect(t('home.play')).toBe('Spielen');
  });

  it('tracks installs, server pings, navigation and toasts', () => {
    applyInstallProgress({ profileId: 'p', projectId: 'x', done: 2, total: 5, label: 'X' });
    expect(installs['x'].done).toBe(2);
    applyInstallProgress({ profileId: 'p', projectId: 'x', done: 5, total: 5, label: 'X' });
    expect(installs['x']).toBeUndefined();
    applyPinged({ id: 's1', online: true, ping: 20, motd: '', players: null, version: null, versionRange: null, iconUrl: null });
    expect(servers.status['s1'].ping).toBe(20);
    openProfile('abc', 'mods');
    expect(nav).toMatchObject({ view: 'profile', profileId: 'abc', profileTab: 'mods' });
    openDiscover('abc', 'shader');
    expect(nav).toMatchObject({ view: 'discover', discoverProfile: 'abc', discoverType: 'shader' });
    toast('hi', 'ok', 1000);
    expect(toasts).toHaveLength(1);
    vi.advanceTimersByTime(1100);
    expect(toasts).toHaveLength(0);
    applyState('g', 'running');
    expect(anyGameActive()).toBe(true);
  });

  it('follows update events and servers via the api', async () => {
    const api = createMockApi();
    await init(api);
    expect(updates.status.state).toBe('idle');
    await actions.updateCheck();
    await vi.advanceTimersByTimeAsync(5000);
    expect(updates.status.state).toBe('ready');
    await actions.loadServers();
    expect(servers.list.length).toBeGreaterThan(3);
    await vi.advanceTimersByTimeAsync(4000);
    expect(Object.keys(servers.status)).toHaveLength(servers.list.length);
  });
});
