import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createMockApi, mockOptionsFromSearch } from './mockApi';
import { SAMPLES } from './mockPortability';
import {
  backToPlan, confirmImport, handleDropped, offerCrash, openCrash, openExport, pf, pickAndImport, renameImport, resetPortability, showModInContent,
} from './portability.svelte';
import { actions, app, flow, init, installs, launches, nav, resetStores, runs, toasts } from './stores.svelte';
import { activityItems } from './activity';

const tick = async (ms = 0) => { await vi.advanceTimersByTimeAsync(ms); };

describe('part 9 flows against the mock', () => {
  beforeEach(() => { vi.useFakeTimers(); resetStores(); resetPortability(); });
  afterEach(() => vi.useRealTimers());

  it('files:dropped starts the import plan for the first modpack and says what it ignored', async () => {
    const api = createMockApi({ timeScale: 0.01 });
    await init(api);
    api.dev.drop(['/home/mango/Downloads/notes.txt', SAMPLES[0], '/home/mango/Downloads/b.zip']);
    await tick(50);
    expect(pf.impOpen).toBe(true);
    expect(pf.imp).toMatchObject({ s: 'plan', path: SAMPLES[0] });
    expect(toasts.some((x) => /1 more file was ignored|weitere Datei/.test(x.text))).toBe(true);
  });

  it('a drop without any modpack only toasts', async () => {
    const api = createMockApi({ timeScale: 0.01 });
    await init(api);
    api.dev.drop(['/home/mango/Downloads/notes.txt']);
    await tick(50);
    expect(pf.imp).toBeNull();
    expect(toasts).toHaveLength(1);
  });

  it('the three kinds produce their plans; CurseForge carries blocked files', async () => {
    const api = createMockApi({ timeScale: 0.01 });
    await init(api);
    const kinds: string[] = [];
    for (const path of SAMPLES) {
      resetPortability();
      handleDropped([path]);
      await tick(50);
      expect(pf.imp?.s).toBe('plan');
      if (pf.imp?.s === 'plan') kinds.push(`${pf.imp.plan.kind}:${pf.imp.plan.blockedFiles.length}`);
    }
    expect(kinds).toEqual(['mrpack:0', 'curseforge:5', 'prism:0']);
  });

  it('imports with progress, adds and selects the instance and navigates to it', async () => {
    const api = createMockApi({ timeScale: 0.01 });
    await init(api);
    const before = app.profiles.length;
    handleDropped([SAMPLES[0]]);
    await tick(50);
    renameImport('Imported Pack');
    const seen: number[] = [];
    const stop = await api.listen('install:progress', (e) => { if (e.projectId === 'import') seen.push(e.done); });
    const done = confirmImport();
    expect(pf.imp?.s).toBe('importing');
    await tick(200);
    await done;
    stop();
    expect(seen.length).toBeGreaterThan(3);
    expect(app.profiles).toHaveLength(before + 1);
    expect(app.profiles.at(-1)?.name).toBe('Imported Pack');
    expect(nav.view).toBe('profile');
    expect(pf.imp).toBeNull();
    expect(installs['import']).toBeUndefined();
  });

  it('a CurseForge import with pending files opens the blocked-files dialog', async () => {
    const api = createMockApi({ timeScale: 0.01 });
    await init(api);
    handleDropped([SAMPLES[1]]);
    await tick(50);
    const done = confirmImport();
    await tick(200);
    await done;
    expect(flow.req).toMatchObject({ kind: 'blocked' });
    expect(app.profiles.at(-1)?.pendingBlocked).toBe(3);
  });

  it('errors are kept as codes for the dialog: path.not_granted on the plan, a failing import can go back', async () => {
    const api = createMockApi({ timeScale: 0.01, importError: 'path.not_granted' });
    await init(api);
    handleDropped([SAMPLES[0]]);
    await tick(50);
    expect(pf.imp).toMatchObject({ s: 'error', code: 'path.not_granted', during: 'plan' });

    const api2 = createMockApi({ timeScale: 0.01 });
    await init(api2);
    resetPortability();
    handleDropped(['/home/mango/Downloads/broken.zip']);
    await tick(50);
    expect(pf.imp).toMatchObject({ s: 'error', code: 'import.not_an_archive' });
    resetPortability();
    handleDropped(['/home/mango/Downloads/huge.zip']);
    await tick(50);
    expect(pf.imp).toMatchObject({ s: 'error', code: 'import.too_large' });
    resetPortability();
    handleDropped(['/home/mango/Downloads/whatever.zip']);
    await tick(50);
    expect(pf.imp).toMatchObject({ s: 'error', code: 'import.unsupported_format' });
    backToPlan();
    expect(pf.imp?.s).toBe('error');
  });

  it('a running import shows up in the activity indicator under a readable title', async () => {
    const api = createMockApi({ timeScale: 0.01 });
    await init(api);
    handleDropped([SAMPLES[0]]);
    await tick(50);
    const done = confirmImport();
    await tick(10);
    const items = activityItems({ profiles: app.profiles, launches, installs, runs, label: (k) => k });
    expect(items.find((i) => i.key === 'inst:import')).toMatchObject({ title: 'import.title', detail: 'Fabulously Optimized.mrpack' });
    await tick(200);
    await done;
  });

  it('a second drop while an import runs does not replace it', async () => {
    const api = createMockApi({ timeScale: 0.01 });
    await init(api);
    handleDropped([SAMPLES[0]]);
    await tick(50);
    const done = confirmImport();
    handleDropped([SAMPLES[2]]);
    await tick(5);
    expect(pf.imp).toMatchObject({ s: 'importing', path: SAMPLES[0] });
    await tick(200);
    await done;
  });

  it('the Import button uses the file dialog; a cancelled dialog does nothing', async () => {
    const api = createMockApi({ timeScale: 0.01 });
    await init(api);
    const pick = vi.spyOn(api, 'pickImportFile');
    pick.mockResolvedValueOnce(null);
    await pickAndImport();
    expect(pf.imp).toBeNull();
    const second = pickAndImport();
    await tick(50);
    await second;
    expect(pf.imp?.s).toBe('plan');
  });

  it('export returns the service warnings; an invalid path is an error code', async () => {
    const api = createMockApi({ timeScale: 0.01 });
    await init(api);
    const id = app.profiles[0].id;
    const run = api.instanceExport({ profileId: id, format: 'mrpack', includeConfig: true, outPath: '/home/mango/Documents/x.mrpack' });
    await tick(100);
    const r = await run;
    expect(r.warnings.length).toBeGreaterThan(1);
    expect(r.warnings.some((w) => /options\.txt/.test(w))).toBe(true);
    await expect(api.instanceExport({ profileId: id, format: 'mrpack', includeConfig: false, outPath: 'rel.mrpack' })).rejects.toMatchObject({ code: 'export.invalid_path' });
    openExport(id);
    expect(pf.exp).toEqual({ profileId: id });
  });

  it('a vanilla instance exports without warnings', async () => {
    const api = createMockApi({ timeScale: 0.01 });
    await init(api);
    const vanilla = app.profiles.find((p) => p.loader === 'vanilla')!;
    const run = api.instanceExport({ profileId: vanilla.id, format: 'mrpack', includeConfig: false, outPath: '/x.mrpack' });
    await tick(100);
    expect((await run).warnings).toEqual([]);
  });

  it('three crash scenarios give different top causes; none gives an empty report', async () => {
    const top: string[] = [];
    for (const crash of ['java', 'oom', 'dependency', 'none'] as const) {
      const api = createMockApi({ timeScale: 0.01, crash });
      await init(api);
      const p = api.crashAnalyze({ profileId: app.profiles[0].id });
      await tick(50);
      const r = await p;
      top.push(r.causes[0]?.code ?? 'none');
    }
    expect(top).toEqual(['java_version', 'out_of_memory', 'mod_dependency', 'none']);
  });

  it('an instance that was never played has no log to analyze', async () => {
    const api = createMockApi({ timeScale: 0.01 });
    await init(api);
    const fresh = await api.profileCreate({ name: 'Fresh', mcVersion: '1.21.4', loader: 'vanilla' });
    const p = api.crashAnalyze({ profileId: fresh.id });
    await tick(50);
    expect((await p).sourceLog).toBeNull();
  });

  it('a crashed launch offers the analysis once, as a toast that opens the panel', async () => {
    const api = createMockApi({ timeScale: 0.01, crash: 'oom' });
    await init(api);
    const id = app.profiles[0].id;
    void actions.play(id);
    await tick(200);
    const t = toasts.find((x) => x.action);
    expect(t).toBeTruthy();
    t?.action?.run();
    expect(pf.crash).toEqual({ profileId: id });
    const n = toasts.length;
    offerCrash(id); // the panel is open: no second offer
    expect(toasts).toHaveLength(n);
  });

  it('openCrash remembers the log; showModInContent filters the Content tab to that mod', () => {
    openCrash('p1', 'crash:x.txt');
    expect(pf.crash).toEqual({ profileId: 'p1', logId: 'crash:x.txt' });
    showModInContent('p1', 'Sodium');
    expect(nav).toMatchObject({ view: 'profile', profileId: 'p1', profileTab: 'content', contentQuery: 'Sodium' });
  });

  it('?mockCrash and ?mockDrop are parsed', () => {
    expect(mockOptionsFromSearch('?mockCrash=oom&mockDrop=curseforge&mockImportError=path.not_granted')).toMatchObject({ crash: 'oom', drop: 'curseforge', importError: 'path.not_granted' });
    expect(mockOptionsFromSearch('?mockCrash=bogus').crash).toBeUndefined();
    expect(mockOptionsFromSearch('?mockDrop').drop).toBe('mrpack');
  });
});

describe('groups and tags through profile_update', () => {
  beforeEach(() => { resetStores(); resetPortability(); });
  it('persists group and tags, normalizes like the service and rejects invalid input', async () => {
    const api = createMockApi({ timeScale: 0.01 });
    await init(api);
    const id = app.profiles[3].id;
    const p = await actions.updateProfile(id, { group: '  Tests  ', tags: ['Alpha', ' alpha ', 'Beta'] });
    expect(p.group).toBe('Tests');
    expect(p.tags).toEqual(['Alpha', 'Beta']);
    expect(app.profiles.find((x) => x.id === id)?.group).toBe('Tests');
    expect((await actions.updateProfile(id, { group: '   ' })).group).toBeNull();
    await expect(actions.updateProfile(id, { group: 'g'.repeat(49) })).rejects.toMatchObject({ code: 'validation' });
    await expect(actions.updateProfile(id, { tags: ['x'.repeat(25)] })).rejects.toMatchObject({ code: 'validation' });
    await expect(actions.updateProfile(id, { tags: Array.from({ length: 13 }, (_, i) => `t${i}`) })).rejects.toMatchObject({ code: 'validation' });
  });
  it('seeded instances have groups and tags', async () => {
    const api = createMockApi();
    await init(api);
    expect(app.profiles.some((p) => p.group)).toBe(true);
    expect(app.profiles.some((p) => !p.group)).toBe(true);
    expect(app.profiles.some((p) => p.tags.length)).toBe(true);
  });
});
