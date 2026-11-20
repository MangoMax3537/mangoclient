import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createMockApi } from './mockApi';
import { cfIdOf } from './mockCatalog';
import { hasKey } from './i18n';
import {
  allBlocked, effectiveProvider, fileCount, includeFor, nextIndex, planBytes, planGroups, preselect, REASON_KEY, requestedBlocked, runSteps,
  selectAll, selectionSummary, sourceOptions, suggestInstance, targetRows, toggleSelection, updateRows, updatable, updateSteps,
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
    const t = await settle(api.installTargets({ provider: 'modrinth', projectId: 'sodium' }), 300);
    const of = (id: string) => t.find((x) => x.profileId === id) as InstallTarget;
    expect(of(iron.id)).toMatchObject({ state: 'installed', installedVersion: '0.6.9' });
    expect(of(neo.id)).toMatchObject({ state: 'available', reason: null });
    expect(of(vanilla.id)).toMatchObject({ state: 'incompatible', reason: 'loader' });
    expect(of(forge120.id)).toMatchObject({ state: 'incompatible', reason: 'no_compatible_version' });
    expect(of(bedwars.id).state).toBe('incompatible');
    // wrong Minecraft version: Create Fabric is 1.21.1 only, Iron Hollow runs Fabric on 1.21.4
    const cf = await settle(api.installTargets({ provider: 'modrinth', projectId: 'create-fabric' }), 300);
    expect(cf.find((x) => x.profileId === iron.id)).toMatchObject({ state: 'incompatible', reason: 'mc_version' });
  });

  it('rows are ordered available, installed, incompatible; only available ones are selectable; every reason has a translation', async () => {
    const { api, profiles } = await setup();
    const rows = targetRows(await settle(api.installTargets({ provider: 'modrinth', projectId: 'sodium' }), 300), profiles);
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

  it('blocked-for-everyone detection, arrow keys and the suggested new instance', () => {
    const rows = [{ target: target('a', { state: 'incompatible', reason: 'distribution_blocked' }), profile: {} as ProfileView, selectable: false, dark: true }];
    expect(allBlocked(rows)).toBe(true);
    expect(allBlocked([])).toBe(false);
    expect(nextIndex(0, 'ArrowDown', 3)).toBe(1);
    expect(nextIndex(2, 'ArrowDown', 3)).toBe(2);
    expect(nextIndex(0, 'ArrowUp', 3)).toBe(0);
    expect(nextIndex(1, 'End', 3)).toBe(2);
    expect(nextIndex(1, 'x', 3)).toBeNull();
    expect(suggestInstance({ loaders: ['forge', 'neoforge'], gameVersions: ['1.21.1', '1.20.1'] })).toEqual({ loader: 'neoforge', mcVersion: '1.21.1' });
    expect(suggestInstance({ loaders: ['iris'], gameVersions: [] })).toEqual({ loader: 'vanilla', mcVersion: '' });
  });
});

describe('providers', () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it('both sources are always offered; CurseForge is disabled with its reason when there is no key', () => {
    expect(sourceOptions(null).every((o) => !o.disabled)).toBe(true);
    const opts = sourceOptions([{ provider: 'modrinth', available: true }, { provider: 'curseforge', available: false, reason: 'no_api_key' }]);
    expect(opts).toEqual([{ provider: 'modrinth', disabled: false, reason: null }, { provider: 'curseforge', disabled: true, reason: 'no_api_key' }]);
    expect(effectiveProvider('curseforge', opts)).toBe('modrinth');
    expect(effectiveProvider('modrinth', opts)).toBe('modrinth');
    expect(effectiveProvider('curseforge', sourceOptions([{ provider: 'curseforge', available: true }]))).toBe('curseforge');
    expect(sourceOptions([{ provider: 'curseforge', available: false }])[1].reason).toBe('offline');
  });

  it('the mock without an API key reports it, and every CurseForge command fails with curseforge.no_api_key', async () => {
    const { api } = await setup({ noCurseforge: true });
    const st = await api.providersStatus();
    expect(st).toContainEqual({ provider: 'curseforge', available: false, reason: 'no_api_key' });
    await expect(api.searchProjects({ params: { provider: 'curseforge', query: '', projectType: 'mod', sort: 'relevance', offset: 0, limit: 5 } })).rejects.toMatchObject({ code: 'curseforge.no_api_key' });
    const ok = await settle(api.searchProjects({ params: { provider: 'modrinth', query: '', projectType: 'mod', sort: 'relevance', offset: 0, limit: 5 } }), 300);
    expect(ok.hits.length).toBe(5);
    expect(ok.hits.every((h) => h.provider === 'modrinth')).toBe(true);
    // saving a key turns it on (config_set accepts curseforgeApiKey and never returns it)
    const cfg = await api.configSet({ patch: { curseforgeApiKey: 'abcdefghijklmnop1234' } });
    expect('curseforgeApiKey' in cfg).toBe(false);
    expect(await api.providersStatus()).toContainEqual({ provider: 'curseforge', available: true });
  });

  it('results are per source: CurseForge ids are numeric strings, no merged list', async () => {
    const { api } = await setup();
    const cf = await settle(api.searchProjects({ params: { provider: 'curseforge', query: 'sodium', projectType: 'mod', sort: 'relevance', offset: 0, limit: 5 } }), 300);
    expect(cf.hits[0]).toMatchObject({ provider: 'curseforge', projectId: cfIdOf('sodium') });
    expect(/^\d+$/.test(cf.hits[0].projectId)).toBe(true);
    const only = await settle(api.searchProjects({ params: { provider: 'modrinth', query: 'compass', projectType: 'mod', sort: 'relevance', offset: 0, limit: 5 } }), 300);
    expect(only.hits).toHaveLength(0); // CurseForge-only project
  });
});

describe('dependency plan to install_apply', () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it('required are locked in, optional only when chosen, installed ones are never sent; counts and bytes follow the choice', async () => {
    const { api, neo } = await setup();
    const plan = await settle(api.installPlan({ provider: 'modrinth', projectId: 'iris', profileId: neo.id }), 300);
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
    const plan = await settle(api.installPlan({ provider: 'modrinth', projectId: 'sodium', profileId: neo.id }), 300);
    expect(planGroups(plan).optional.map((i) => i.projectId)).toEqual(['sodium-extra', 'reeses-sodium-options']);
    const spy = vi.spyOn(api, 'installApply');
    const res = settle(runSteps(api, [{ profileId: neo.id, provider: 'modrinth', projectId: 'sodium', include: includeFor(plan, ['sodium-extra']), title: 'Sodium' }]), 4000);
    const [r] = await res;
    expect(spy).toHaveBeenCalledWith({ provider: 'modrinth', projectId: 'sodium', profileId: neo.id, include: ['sodium-extra'] });
    expect(r.ok).toBe(true);
    const ids = (r.items ?? []).map((i) => i.projectId);
    expect(ids).toEqual(expect.arrayContaining(['sodium', 'sodium-extra']));
    expect(ids).not.toContain('reeses-sodium-options');
  });

  it('installed dependencies are marked and not downloaded again; conflicts are reported', async () => {
    const { api, iron } = await setup();
    const plan = await settle(api.installPlan({ provider: 'modrinth', projectId: 'cull-leaves', profileId: iron.id }), 300);
    expect(plan.conflicts).toEqual([{ projectId: 'moreculling', title: 'More Culling', with: 'Cull Leaves' }]);
    const z = await settle(api.installPlan({ provider: 'modrinth', projectId: 'ftb-quests', profileId: iron.id }), 300);
    const req = planGroups(z).required;
    expect(req.map((d) => d.projectId)).toEqual(['architectury']);
    expect(req.every((d) => d.alreadyInstalled)).toBe(true);
    expect(z.totalBytes).toBe(z.items.find((i) => i.role === 'requested')?.sizeBytes);
  });

  it('a distribution-blocked CurseForge item carries the website URL, is skipped as dependency and refused as the requested item', async () => {
    const { api, neo } = await setup();
    const rift = await settle(api.searchProjects({ params: { provider: 'curseforge', query: 'compass', projectType: 'mod', sort: 'relevance', offset: 0, limit: 3 } }), 300);
    const plan = await settle(api.installPlan({ provider: 'curseforge', projectId: rift.hits[0].projectId, profileId: neo.id }), 300);
    const blocked = planGroups(plan).blocked;
    expect(blocked).toHaveLength(1);
    expect(blocked[0].downloadUrl).toMatch(/^https:\/\/www\.curseforge\.com\/minecraft\//);
    expect(blocked[0].sizeBytes).toBeNull();
    expect(requestedBlocked(plan)).toBe(false);
    expect(fileCount(plan, [])).toBe(2); // Rift Compass + Fabric API, not the blocked Waystone Atlas
    const atlas = await settle(api.installTargets({ provider: 'curseforge', projectId: cfIdOf('waystone-atlas') }), 300);
    expect(atlas.every((t) => t.reason === 'distribution_blocked')).toBe(true);
    const bp = await settle(api.installPlan({ provider: 'curseforge', projectId: cfIdOf('waystone-atlas'), profileId: neo.id }), 300);
    expect(requestedBlocked(bp)).toBe(true);
    const [r] = await settle(runSteps(api, [{ profileId: neo.id, provider: 'curseforge', projectId: cfIdOf('waystone-atlas'), include: [], title: 'Waystone Atlas' }]), 500);
    expect(r).toMatchObject({ ok: false, code: 'distribution_blocked' });
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
    const out = await runSteps(api, ['a', 'b', 'c'].map((profileId) => ({ profileId, provider: 'modrinth' as const, projectId: 'x', include: [], title: 'X' })), { onResult: (i, r) => seen.push(`${i}:${r.ok}`) });
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
    expect(infos.every((i) => i.provider === 'modrinth')).toBe(true);
    const plans: (InstallPlan | null)[] = await settle(Promise.all(infos.map((i) => api.installPlan({ provider: 'modrinth', projectId: i.projectId, versionId: i.versionId, profileId: iron.id }))), 500);
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

  it('updates whose requested file is blocked are not applied', () => {
    const info = { provider: 'curseforge' as const, projectId: '1', title: 'X', type: 'mod' as const, from: '1', to: '2', versionId: 'v', compatibility: true, conflict: false, requiredBy: [] };
    const plan: InstallPlan = { profileId: 'p', totalBytes: 0, conflicts: [], items: [{ provider: 'curseforge', projectId: '1', versionId: 'v', title: 'X', type: 'mod', iconUrl: null, role: 'requested', alreadyInstalled: false, installedVersion: '1', distributionBlocked: true, sizeBytes: null, downloadUrl: 'https://www.curseforge.com/minecraft/mc-mods/x' }] };
    const rows = updateRows([info], [plan]);
    expect(updatable(rows)).toHaveLength(0);
    expect(updateSteps('p', rows)).toEqual([]);
  });
});

describe('external links for blocked files', () => {
  it('opens CurseForge Minecraft pages only', async () => {
    const { isAllowedExternalUrl } = await import('./auth');
    expect(isAllowedExternalUrl('https://www.curseforge.com/minecraft/mc-mods/waystone-atlas')).toBe(true);
    expect(isAllowedExternalUrl('https://curseforge.com/minecraft')).toBe(true);
    expect(isAllowedExternalUrl('https://www.curseforge.com/account')).toBe(false);
    expect(isAllowedExternalUrl('https://www.curseforge.com/minecraftfake')).toBe(false);
    expect(isAllowedExternalUrl('https://evilcurseforge.com/minecraft/x')).toBe(false);
    expect(isAllowedExternalUrl('http://www.curseforge.com/minecraft/x')).toBe(false);
  });
});

describe('modpack flow', () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it('modpack_plan lists blocked files with website links for both providers; every instance is content_type for the picker', async () => {
    const { api, profiles } = await setup();
    const cf = await settle(api.searchProjects({ params: { provider: 'curseforge', query: 'prominence', projectType: 'modpack', sort: 'relevance', offset: 0, limit: 3 } }), 300);
    const plan = await settle(api.modpackPlan({ provider: 'curseforge', projectId: cf.hits[0].projectId }), 400);
    expect(plan).toMatchObject({ name: 'Prominence II', mcVersion: '1.20.1', loader: 'forge', fileCount: 268 });
    expect(plan.blockedFiles).toHaveLength(3);
    expect(plan.blockedFiles.every((f) => f.url.startsWith('https://www.curseforge.com/minecraft/'))).toBe(true);
    const mr = await settle(api.modpackPlan({ provider: 'modrinth', projectId: 'fabulously-optimized' }), 400);
    expect(mr.blockedFiles.length).toBeGreaterThan(0);
    const t = await settle(api.installTargets({ provider: 'modrinth', projectId: 'fabulously-optimized' }), 300);
    expect(t).toHaveLength(profiles.length);
    expect(t.every((x) => x.state === 'incompatible' && x.reason === 'content_type')).toBe(true);
  });

  it('modpack_install with a custom name creates the instance, emits progress with the new profile id and ends done', async () => {
    const { api } = await setup();
    const ev: { profileId: string; done: number; total: number }[] = [];
    await api.listen('install:progress', (e) => ev.push(e));
    const p = await settle(api.modpackInstall({ provider: 'modrinth', projectId: 'fabulously-optimized', name: 'My FO' }), 3000);
    expect(p.name).toBe('My FO');
    expect(p.mcVersion).toBe('1.21.4');
    expect(ev.length).toBeGreaterThan(3);
    expect(ev.every((e) => e.profileId === p.id)).toBe(true);
    expect(ev.at(-1)?.done).toBe(ev.at(-1)?.total);
    expect((await api.appState()).profiles.some((x) => x.id === p.id)).toBe(true);
  });
});
