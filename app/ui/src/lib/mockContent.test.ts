import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createMockApi } from './mockApi';
import type { Api } from './api';
import type { ApiEvents } from './types';

async function setup() {
  const api = createMockApi();
  const s = await api.appState();
  return { api, iron: s.profiles[0].id, vanilla: s.profiles[1].id, shader: s.profiles[3].id };
}
const settle = async <T>(p: Promise<T>, ms = 3000): Promise<T> => { await vi.advanceTimersByTimeAsync(ms); return p; };

describe('mockApi part 2', () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it('lists 38 content items incl. dependencies and updates', async () => {
    const { api, iron } = await setup();
    const items = await settle(api.contentList({ profileId: iron }), 200);
    expect(items).toHaveLength(38);
    expect(new Set(items.map((i) => i.type))).toEqual(new Set(['mod', 'shader', 'resourcepack']));
    expect(items.find((i) => i.projectId === 'sodium')?.requiredBy).toEqual(expect.arrayContaining(['iris', 'sodium-extra']));
    expect(items.some((i) => i.dependency)).toBe(true);
    expect(items.filter((i) => i.updateAvailable).length).toBeGreaterThanOrEqual(4);
    const ups = await settle(api.contentCheckUpdates({ profileId: iron }), 400);
    expect(ups.map((u) => u.projectId)).toEqual(expect.arrayContaining(['sodium', 'iris']));
  });

  it('toggle cascades: disabling a library disables dependents, enabling restores requirements', async () => {
    const { api, iron } = await setup();
    const off = await api.contentToggle({ profileId: iron, projectId: 'sodium', enabled: false });
    expect(off.affected).toEqual(expect.arrayContaining(['iris', 'sodium-extra', 'reeses-sodium-options']));
    expect(off.items.find((i) => i.projectId === 'iris')?.enabled).toBe(false);
    const on = await api.contentToggle({ profileId: iron, projectId: 'iris', enabled: true });
    expect(on.affected).toContain('sodium');
    expect(on.items.find((i) => i.projectId === 'sodium')?.enabled).toBe(true);
  });

  it('uninstall is blocked by dependents and succeeds for leaves', async () => {
    const { api, iron } = await setup();
    const blocked = await api.contentUninstall({ profileId: iron, projectId: 'sodium' });
    expect(blocked.blocked).toBe(true);
    expect(blocked.dependents).toContain('iris');
    expect(blocked.items).toHaveLength(38);
    const ok = await api.contentUninstall({ profileId: iron, projectId: 'iris' });
    expect(ok).toMatchObject({ blocked: false, dependents: [] });
    expect(ok.items).toHaveLength(37);
    expect(ok.items.find((i) => i.projectId === 'sodium')?.requiredBy).not.toContain('iris');
  });

  it('search pages through 40+ mod hits and filters', async () => {
    const { api } = await setup();
    const base = { query: '', projectType: 'mod' as const, sort: 'downloads' as const, limit: 20 };
    const p1 = await settle(api.modrinthSearch({ params: { ...base, offset: 0 } }), 300);
    const p2 = await settle(api.modrinthSearch({ params: { ...base, offset: 20 } }), 300);
    const p3 = await settle(api.modrinthSearch({ params: { ...base, offset: 40 } }), 300);
    expect(p1.total).toBeGreaterThanOrEqual(40);
    expect(p1.hits).toHaveLength(20);
    expect(p2.hits).toHaveLength(20);
    expect(p1.hits[0].downloads).toBeGreaterThanOrEqual(p1.hits[1].downloads);
    expect(new Set([...p1.hits, ...p2.hits, ...p3.hits].map((h) => h.projectId)).size).toBe(p1.hits.length + p2.hits.length + p3.hits.length);
    const q = await settle(api.modrinthSearch({ params: { ...base, query: 'sodium', offset: 0 } }), 300);
    expect(q.hits.every((h) => /sodium/i.test(`${h.title} ${h.description}`))).toBe(true);
    const sh = await settle(api.modrinthSearch({ params: { ...base, projectType: 'shader', offset: 0 } }), 300);
    expect(sh.hits.every((h) => h.type === 'shader')).toBe(true);
    expect(sh.hits[0].installedIn.length).toBeGreaterThan(0);
  });

  it('install emits progress events, adds required dependencies and returns the full list', async () => {
    const { api, iron, vanilla } = await setup();
    const ev: ApiEvents['install:progress'][] = [];
    await api.listen('install:progress', (e) => ev.push(e));
    const p = api.modrinthInstall({ profileId: iron, projectId: 'create-fabric' });
    await vi.advanceTimersByTimeAsync(200);
    expect(ev.length).toBeGreaterThan(0);
    expect(ev.length).toBeLessThan(5);
    const items = await settle(p, 2000);
    expect(ev.at(-1)).toMatchObject({ profileId: iron, projectId: 'create-fabric', done: 5, total: 5 });
    expect(items).toHaveLength(39);
    expect(items.find((i) => i.projectId === 'create-fabric')?.enabled).toBe(true);
    const v = await settle(api.modrinthInstall({ profileId: vanilla, projectId: 'create-fabric' }), 2000);
    expect(v.map((i) => i.projectId)).toEqual(expect.arrayContaining(['create-fabric', 'fabric-api']));
    expect(v.find((i) => i.projectId === 'fabric-api')?.dependency).toBe(true);
  });

  it('modpack install creates a profile', async () => {
    const { api } = await setup();
    const p = await settle(api.modrinthInstallModpack({ projectId: 'fabulously-optimized' }), 3000);
    expect(p.name).toBe('Fabulously Optimized');
    expect((await api.appState()).profiles.some((x) => x.id === p.id)).toBe(true);
    expect((await settle(api.contentList({ profileId: p.id }), 200)).length).toBeGreaterThan(0);
  });

  it('versions mark compatibility per profile', async () => {
    const { api, iron } = await setup();
    const v = await settle(api.modrinthVersions({ projectId: 'sodium', profileId: iron }), 300);
    expect(v.some((x) => x.compatible)).toBe(true);
    expect(v.some((x) => !x.compatible)).toBe(true);
  });

  it('servers ping events cover every server, mixed online/offline', async () => {
    const api: Api = createMockApi();
    const list = await api.serversList();
    const got: ApiEvents['servers:pinged'][] = [];
    await api.listen('servers:pinged', (s) => got.push(s));
    await api.serversPingAll();
    expect(got).toHaveLength(0); // returns immediately, results arrive as events
    await vi.advanceTimersByTimeAsync(4000);
    expect(got.map((g) => g.id).sort()).toEqual(list.map((s) => s.id).sort());
    expect(got.some((g) => g.online)).toBe(true);
    expect(got.some((g) => !g.online)).toBe(true);
    expect(list.some((s) => s.partner && s.starred)).toBe(true);
  });

  it('media, logs, stats, storage', async () => {
    const { api, iron } = await setup();
    const shots = await api.screenshotsList({ profileId: iron });
    expect(shots).toHaveLength(12);
    await api.screenshotAction({ profileId: iron, name: shots[0].name, action: 'delete' });
    expect(await api.screenshotsList({ profileId: iron })).toHaveLength(11);
    const logs = await api.logsList({ profileId: iron });
    expect(logs.some((l) => l.kind === 'crash')).toBe(true);
    const crash = await api.logsRead({ profileId: iron, id: logs.find((l) => l.kind === 'crash')!.id });
    expect(crash.text).toContain('Caused by');
    const up = await settle(api.logsUpload({ profileId: iron, id: logs[0].id }), 1000);
    expect(up.url).toMatch(/^https:\/\/mclo\.gs\//);
    const s90 = await settle(api.statsSummary({ days: 90 }), 200);
    expect(s90.days).toHaveLength(90);
    expect(s90.totalMs).toBe(s90.days.reduce((a, d) => a + d.ms, 0));
    expect((await settle(api.statsSummary({ days: 14 }), 200)).days).toHaveLength(14);
    const st = await api.storageUsage();
    expect(st.map((x) => x.key)).toContain('assets');
    const after = await settle(api.storageClear({ key: 'cache' }), 400);
    expect(after.find((x) => x.key === 'cache')?.bytes).toBe(0);
  });

  it('storage_clear is refused while a game runs', async () => {
    const { api, iron } = await setup();
    void api.gameLaunch({ profileId: iron });
    await vi.advanceTimersByTimeAsync(6500);
    await expect(api.storageClear({ key: 'logs' })).rejects.toMatchObject({ code: 'game_running' });
  });

  it('update flow goes idle -> checking -> downloading -> ready and install returns true', async () => {
    const api = createMockApi();
    expect((await api.updateStatus()).state).toBe('idle');
    const seen: string[] = [];
    const pct: number[] = [];
    await api.listen('update:status', (u) => { seen.push(u.state); if (u.percent !== undefined) pct.push(u.percent); });
    expect(await api.updateInstall()).toBe(false);
    await api.updateCheck();
    await vi.advanceTimersByTimeAsync(5000);
    expect([...new Set(seen)]).toEqual(['checking', 'downloading', 'ready']);
    expect(pct[0]).toBe(0);
    expect(pct.at(-1)).toBe(100);
    expect(await api.updateStatus()).toMatchObject({ state: 'ready', version: '2.0.1' });
    expect(await api.updateInstall()).toBe(true);
  });

  it('accounts carry a headUrl', async () => {
    const { api } = await setup();
    for (const a of (await api.appState()).accounts) expect(a.headUrl).toBeTruthy();
  });
});
