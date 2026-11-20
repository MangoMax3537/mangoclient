import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createMockApi } from './mockApi';
import { activityItems } from './activity';
import {
  app, applyTheme, closeFlow, contentRev, errorText, flow, init, installs, launches, nav, resetStores, runs, startContentRun, startInstall,
  startModpackRun, t, theme, toasts,
} from './stores.svelte';

async function boot(opts = {}) {
  resetStores();
  const api = createMockApi(opts);
  await init(api);
  const by = (n: string) => app.profiles.find((p) => p.name === n)!;
  return { api, neo: by('Create Expedition'), iron: by('Iron Hollow') };
}

describe('install flow store', () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => { vi.useRealTimers(); resetStores(); });

  it('a background run tracks per-instance status, bumps content revisions and ends with a summary toast', async () => {
    const { neo, iron } = await boot();
    const run = startContentRun('content', 'Sodium', null, [
      { profileId: neo.id, projectId: 'sodium', include: [], title: 'Sodium' },
      { profileId: iron.id, projectId: 'create-fabric', include: [], title: 'Sodium' },
    ]);
    expect(run.jobs.map((j) => j.status)).toEqual(['running', 'queued']); // sequential: the second waits
    await vi.advanceTimersByTimeAsync(10);
    expect(run.jobs.map((j) => j.status)).toEqual(['running', 'queued']);
    // while it runs the activity list names it
    const act = activityItems({ profiles: app.profiles, launches, installs, runs, label: (k) => k });
    expect(act.some((a) => a.kind === 'install' && a.title === 'Sodium' && a.runId === run.id)).toBe(true);
    await vi.advanceTimersByTimeAsync(5000);
    expect(run.finished).toBe(true);
    expect(run.jobs.map((j) => j.status)).toEqual(['done', 'done']);
    expect(contentRev[neo.id]).toBe(1);
    expect(toasts.at(-1)?.kind).toBe('ok');
    expect(toasts.at(-1)?.text).toContain('2');
  });

  it('a failing step is marked with its code and the summary says how many finished', async () => {
    const { neo } = await boot();
    const run = startContentRun('content', 'Waystone Atlas', null, [
      { profileId: neo.id, projectId: '999999999', include: [], title: 'x' },
    ]);
    await vi.advanceTimersByTimeAsync(2000);
    expect(run.finished).toBe(true);
    expect(run.jobs[0].status).toBe('error');
    expect(run.jobs[0].error).toBe('project_not_found');
    expect(toasts.at(-1)?.kind).toBe('error');
  });

  it('a modpack run navigates to the new instance only while its dialog is open', async () => {
    const { } = await boot();
    const proj = { projectId: 'fabulously-optimized', title: 'Fabulously Optimized', type: 'modpack' as const, iconUrl: null };
    startInstall({ kind: 'modpack', project: proj });
    const a = await startModpackRun(proj, 'FO watched', undefined, true);
    await vi.advanceTimersByTimeAsync(3000);
    const pa = await a.done;
    expect(pa?.name).toBe('FO watched');
    expect(nav).toMatchObject({ view: 'profile', profileId: pa?.id, profileTab: 'mods' });
    expect(flow.req).toBeNull();

    nav.view = 'home'; nav.profileId = null;
    const b = await startModpackRun(proj, 'FO background', undefined, true);
    b.run.watching = false; // dialog closed meanwhile
    await vi.advanceTimersByTimeAsync(3000);
    const pb = await b.done;
    expect(app.profiles.some((p) => p.id === pb?.id)).toBe(true);
    expect(nav.view).toBe('home');
  });

  it('closing the run dialog marks the run as not watched', async () => {
    const { neo } = await boot();
    const run = startContentRun('content', 'Iris', null, [{ profileId: neo.id, projectId: 'iris', include: [], title: 'Iris' }]);
    startInstall({ kind: 'run', runId: run.id });
    closeFlow();
    expect(flow.req).toBeNull();
    expect(runs.find((r) => r.id === run.id)?.watching).toBe(false);
    await vi.advanceTimersByTimeAsync(3000);
  });
});

describe('providers state, errors, theme, activity', () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => { vi.useRealTimers(); resetStores(); });

  it('errors map by code, then by family, then fall back', async () => {
    await boot();
    expect(errorText({ code: 'import.unsupported_curseforge' })).toBe(t('err.import.unsupported_curseforge'));
    expect(errorText({ code: 'net.timeout' })).toBe(t('err.net.timeout'));
    expect(new Set(['net.timeout', 'project_not_found', 'totally.unknown'].map((c) => errorText({ code: c }))).size).toBe(3);
    expect(errorText({ code: 'totally.unknown' })).toBe(t('err.unknown'));
  });

  it('theme mode is kept in the store (system, dark, light)', () => {
    applyTheme('light'); expect(theme.mode).toBe('light');
    applyTheme('system'); expect(theme.mode).toBe('system');
  });

  it('activity lists launches in progress and running games, installs without a run', async () => {
    const { neo } = await boot();
    const label = (k: string) => k;
    launches[neo.id] = { state: 'downloading', progress: null, downloadFraction: 0.5 };
    let a = activityItems({ profiles: app.profiles, launches, installs, runs, label });
    expect(a).toHaveLength(1);
    expect(a[0]).toMatchObject({ kind: 'launch', profileId: neo.id });
    expect(a[0].fraction).toBeCloseTo((3 + 0.5) / 6);
    launches[neo.id] = { state: 'running', progress: null, downloadFraction: 0 };
    installs['x'] = { profileId: neo.id, projectId: 'x', done: 1, total: 4, label: 'Thing' };
    a = activityItems({ profiles: app.profiles, launches, installs, runs, label });
    expect(a.map((i) => i.kind).sort()).toEqual(['install', 'running']);
    expect(a.find((i) => i.kind === 'install')?.fraction).toBe(0.25);
  });
});
