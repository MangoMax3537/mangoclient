import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createMockApi } from './mockApi';
import {
  app, applyProgress, applyState, CONSOLE_CAP, consoles, init, isBusy, launchInfo, pushLog, resetStores, stageProgress, t,
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
    expect(app.profiles).toHaveLength(4);
    const id = app.profiles[0].id;
    void api.gameLaunch({ profileId: id });
    await vi.advanceTimersByTimeAsync(6500);
    expect(launchInfo(id).state).toBe('running');
    expect(consoles[id].length).toBeGreaterThan(0);
    expect(t('home.play')).toBe('Spielen');
  });
});
