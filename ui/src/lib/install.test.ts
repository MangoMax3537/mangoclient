import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createMockApi } from './mockApi';
import { hasKey } from './i18n';
import {
  fileCount, includeFor, nextIndex, planBytes, planGroups, preselect, REASON_KEY, runSteps,
  selectAll, selectionSummary, suggestInstance, targetRows, toggleSelection, updateRows, updateSteps,
} from './install';
import type { Api } from './api';
import type { InstallPlan, InstallTarget, ProfileView } from './types';

const settle = async <T>(p: Promise<T>, ms = 3000): Promise<T> => { await vi.advanceTimersByTimeAsync(ms); return p; };
async function setup(opts = {}) {
  const api = createMockApi(opts);
  const s = await api.appState();
  const by = (n: string) => s.profiles.find((p) => p.name === n) as ProfileView;
  return { api, profiles: s.profiles, iron: by('Iron Hollow'), vanilla: by('Vanilla Survival'), neo: by('Create Expedition'), forge120: by('Classic Forge 1.20.1'), bedwars: by('Bedwars Practice') };
}
const target = (profileId: string, over: Partial<InstallTarget> = {}): InstallTarget => ({ profileId, state: 'available', installedVersion: null, reason: null, detail: null, requiredDeps: 0, ...over });

describe('instance picker mapping', () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it('install_targets of the mock cover every state for Sodium', async () => {
    const { api, iron, vanilla, neo, forge120, bedwars } = await setup();
    const t = await settle(api.installTargets({ projectId: 'sodium' }), 300);
    const of = (id: string) => t.find((x) => x.profileId === id) as InstallTarget;
    expect(of(iron.id)).toMatchObject({ state: 'installed', installedVersion: '0.6.9' });
    expect(of(neo.id)).toMatchObject({ state: 'available', reason: null });
    expect(of(vanilla.id)).toMatchObject({ state: 'incompatible', reason: 'loader' });
    expect(of(forge120.id)).toMatchObject({ state: 'incompatible', reason: 'no_compatible_version' });
    expect(of(bedwars.id).state).toBe('incompatible');
    // wrong Minecraft version: Create Fabric is 1.21.1 only, Iron Hollow runs Fabric on 1.21.4
    const cf = await settle(api.installTargets({ projectId: 'create-fabric' }), 300);
    expect(cf.find((x) => x.profileId === iron.id)).toMatchObject({ state: 'incompatible', reason: 'mc_version' });
  });

  it('rows are ordered available, installed, incompatible; only available ones are selectable; every reason has a translation', async () => {
    const { api, profiles } = await setup();
    const rows = targetRows(await settle(api.installTargets({ projectId: 'sodium' }), 300), profiles);
    expect(rows.map((r) => r.target.state)).toEqual([...rows.map((r) => r.target.state)].sort((a, b) => ['available', 'installed', 'incompatible'].indexOf(a) - ['available', 'installed', 'incompatible'].indexOf(b)));
    expect(rows.every((r) => r.selectable === (r.target.state === 'available') && r.dark === !r.selectable)).toBe(true);
    for (const key of Object.values(REASON_KEY)) { expect(hasKey('en', key)).toBe(true); expect(hasKey('de', key)).toBe(true); }
  });

  it('selection: dark rows cannot be toggled, multi-select works, preselect prefers open then last used then current', async () => {
    const { profiles, iron, neo, vanilla } = await setup();
    const rows = targetRows([target(iron.id, { state: 'installed', installedVersion: '1' }), target(neo.id, { requiredDeps: 2 }), target(vanilla.id, { state: 'incompatible', reason: 'loader' })], profiles);
    expect(toggleSelection([], iron.id, rows)).toEqual([]);
    expect(toggleSelection([], vanilla.id, rows)).toEqual([]);
    expect(toggleSelection([], neo.id, rows)).toEqual([neo.id]);
    expect(toggleSelection([neo.id], neo.id, rows)).toEqual([]);
    expect(preselect(rows, [iron.id, neo.id, null])).toEqual([neo.id]); // the open instance is installed, skip to the next candidate
    expect(preselect(rows, [vanilla.id, undefined])).toEqual([]);
    expect(selectAll(rows)).toEqual([neo.id]);
    expect(selectionSummary(rows, [neo.id, iron.id])).toEqual({ count: 1, requiredDeps: 2, available: 1 });
  });

  it('arrow keys and the suggested new instance', () => {
    expect(nextIndex(0, 'ArrowDown', 3)).toBe(1);
    expect(nextIndex(2, 'ArrowDown', 3)).toBe(2);
    expect(nextIndex(0, 'ArrowUp', 3)).toBe(0);
    expect(nextIndex(1, 'End', 3)).toBe(2);
    expect(nextIndex(1, 'x', 3)).toBeNull();
    expect(suggestInstance({ loaders: ['forge', 'neoforge'], gameVersions: ['1.21.1', '1.20.1'] })).toEqual({ loader: 'neoforge', mcVersion: '1.21.1' });
    expect(suggestInstance({ loaders: ['iris'], gameVersions: [] })).toEqual({ loader: 'vanilla', mcVersion: '' });
  });
});

describe('dependency plan to install_apply', () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it('required are locked in, optional only when chosen, installed ones are never sent; counts and bytes follow the choice', async () => {
    const { api, neo } = await setup();
    const plan = await settle(api.installPlan({ projectId: 'iris', profileId: neo.id }), 300);
    const g = planGroups(plan);
    expect(g.requested?.projectId).toBe('iris');
    expect(g.required.map((i) => i.projectId)).toEqual(['sodium']);
    expect(g.optional.map((i) => i.projectId)).toEqual(['complementary-reimagined']);
    expect(includeFor(plan, [])).toEqual([]);
    expect(includeFor(plan, ['complementary-reimagined', 'sodium', 'bogus'])).toEqual(['complementary-reimagined']);
    expect(fileCount(plan, [])).toBe(2);
    expect(fileCount(plan, ['complementary-reimagined'])).toBe(3);
    expect(planBytes(plan, ['complementary-reimagined'])).toBeGreaterThan(planBytes(plan, []));
    expect(planBytes(plan, [])).toBe(plan.totalBytes);
  });

  it('confirming sends exactly the chosen optional ids to install_apply, and only those end up installed', async () => {
    const { api, neo } = await setup();
    const plan = await settle(api.installPlan({ projectId: 'sodium', profileId: neo.id }), 300);
    expect(planGroups(plan).optional.map((i) => i.projectId)).toEqual(['sodium-extra', 'reeses-sodium-options']);
    const spy = vi.spyOn(api, 'installApply');
    const res = settle(runSteps(api, [{ profileId: neo.id, projectId: 'sodium', include: includeFor(plan, ['sodium-extra']), title: 'Sodium' }]), 4000);
    const [r] = await res;
    expect(spy).toHaveBeenCalledWith({ projectId: 'sodium', profileId: neo.id, include: ['sodium-extra'] });
    expect(r.ok).toBe(true);
    const ids = (r.items ?? []).map((i) => i.projectId);
    expect(ids).toEqual(expect.arrayContaining(['sodium', 'sodium-extra']));
    expect(ids).not.toContain('reeses-sodium-options');
  });

  it('installed dependencies are marked and not downloaded again; conflicts are reported', async () => {
    const { api, iron } = await setup();
    const plan = await settle(api.installPlan({ projectId: 'cull-leaves', profileId: iron.id }), 300);
    expect(plan.conflicts).toEqual([{ projectId: 'moreculling', title: 'More Culling', with: 'Cull Leaves' }]);
    const z = await settle(api.installPlan({ projectId: 'ftb-quests', profileId: iron.id }), 300);
    const req = planGroups(z).required;
    expect(req.map((d) => d.projectId)).toEqual(['architectury']);
    expect(req.every((d) => d.alreadyInstalled)).toBe(true);
    expect(z.totalBytes).toBe(z.items.find((i) => i.role === 'requested')?.sizeBytes);
  });

  it('runSteps is sequential, reports each result and keeps going after a failure', async () => {
    const api = { installApply: vi.fn() } as unknown as Pick<Api, 'installApply'>;
    const order: string[] = [];
    (api.installApply as ReturnType<typeof vi.fn>).mockImplementation(async (a: { profileId: string }) => {
      order.push(`start ${a.profileId}`);
      await Promise.resolve();
      if (a.profileId === 'b') throw { code: 'no_compatible_version', message: '', needsRelogin: false };
      order.push(`end ${a.profileId}`);
      return [];
    });
    const seen: string[] = [];
    const out = await runSteps(api, ['a', 'b', 'c'].map((profileId) => ({ profileId, projectId: 'x', include: [], title: 'X' })), { onResult: (i, r) => seen.push(`${i}:${r.ok}`) });
    expect(order).toEqual(['start a', 'end a', 'start b', 'start c', 'end c']);
    expect(out.map((r) => r.ok)).toEqual([true, false, true]);
    expect(out[1].code).toBe('no_compatible_version');
    expect(seen).toEqual(['0:true', '1:false', '2:true']);
  });
});

describe('update-all preview', () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it('shows versions and new dependencies before anything is applied; apply sends no optional ids', async () => {
    const { api, iron } = await setup();
    const infos = await settle(api.contentCheckUpdates({ profileId: iron.id }), 400);
    const plans: (InstallPlan | null)[] = await settle(Promise.all(infos.map((i) => api.installPlan({ projectId: i.projectId, versionId: i.versionId, profileId: iron.id }))), 500);
    const rows = updateRows(infos, plans);
    const modmenu = rows.find((r) => r.info.projectId === 'modmenu');
    expect(modmenu?.info).toMatchObject({ from: '12.0.0', to: '13.0.0' });
    expect(modmenu?.newDeps.map((d) => d.projectId)).toEqual(['terrablender']);
    expect(rows.find((r) => r.info.projectId === 'sodium')?.newDeps).toEqual([]);
    const steps = updateSteps(iron.id, rows);
    expect(steps).toHaveLength(infos.length);
    expect(steps.every((s) => s.include.length === 0 && s.profileId === iron.id)).toBe(true);
    expect(steps.find((s) => s.projectId === 'iris')?.versionId).toBe('iris-vu');
    // nothing was installed by planning
    expect((await settle(api.contentList({ profileId: iron.id }), 100)).some((i) => i.projectId === 'terrablender')).toBe(false);
    const results = await settle(runSteps(api, steps.slice(0, 3)), 6000);
    expect(results.every((r) => r.ok)).toBe(true);
    expect((await settle(api.contentList({ profileId: iron.id }), 100)).find((i) => i.projectId === 'sodium')?.versionNumber).toBe('0.6.13');
  });
});

describe('modpack flow', () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it('modpack_plan describes the pack; every instance is content_type for the picker', async () => {
    const { api, profiles } = await setup();
    const mr = await settle(api.modpackPlan({ projectId: 'fabulously-optimized' }), 400);
    expect(mr).toMatchObject({ name: 'Fabulously Optimized', mcVersion: '1.21.4', loader: 'fabric' });
    expect(mr).not.toHaveProperty('blockedFiles');
    const t = await settle(api.installTargets({ projectId: 'fabulously-optimized' }), 300);
    expect(t).toHaveLength(profiles.length);
    expect(t.every((x) => x.state === 'incompatible' && x.reason === 'content_type')).toBe(true);
  });

  it('modpack_install with a custom name creates the instance, emits progress with the new profile id and ends done', async () => {
    const { api } = await setup();
    const ev: { profileId: string; done: number; total: number }[] = [];
    await api.listen('install:progress', (e) => ev.push(e));
    const p = await settle(api.modpackInstall({ projectId: 'fabulously-optimized', name: 'My FO' }), 3000);
    expect(p.name).toBe('My FO');
    expect(p.mcVersion).toBe('1.21.4');
    expect(ev.length).toBeGreaterThan(3);
    expect(ev.every((e) => e.profileId === p.id)).toBe(true);
    expect(ev.at(-1)?.done).toBe(ev.at(-1)?.total);
    expect((await api.appState()).profiles.some((x) => x.id === p.id)).toBe(true);
  });
});
