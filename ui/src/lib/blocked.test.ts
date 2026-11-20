import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  canFinishSilently, cleanDir, isAbsolutePath, missingFiles, openAllNeedsConfirm, OPEN_ALL_CONFIRM_THRESHOLD, openPagesSequentially, planSplit,
  planSummaryText, sortBlocked, summarize, validateWatchDir,
} from './blocked';
import { createMockApi } from './mockApi';
import { blockedFilesFor } from './mockCatalog';
import { de6, en6 } from './i18n6';
import { plural } from './i18n';
import { app, blocked, init, loadBlocked, resetStores, skipBlocked, startBlockedWatch, startModpackRun, flow } from './stores.svelte';
import type { BlockedFile } from './types';

const VAULT = 'vault-hunters-reborn';
const file = (over: Partial<BlockedFile>): BlockedFile => ({ name: 'X', fileName: 'x.jar', url: 'https://www.curseforge.com/minecraft/mc-mods/x', sha1: null, size: null, type: 'mod', state: 'pending', modrinth: null, foundPath: null, ...over });

describe('plan summary', () => {
  it('splits 6 blocked files into 2 from Modrinth and 4 to download', () => {
    const files = blockedFilesFor(VAULT);
    expect(files).toHaveLength(6);
    expect(planSplit(files)).toEqual({ total: 6, matched: 2, manual: 4 });
    expect(files.filter((f) => f.state === 'matched_modrinth').every((f) => f.modrinth)).toBe(true);
    expect(files.every((f) => /^[0-9a-f]{40}$/.test(f.sha1 ?? ''))).toBe(true);
  });
  it('reads as one sentence in both languages and leaves out empty parts', () => {
    const files = blockedFilesFor(VAULT);
    const en = (k: string, n: number) => plural('en', k, n);
    const de = (k: string, n: number) => plural('de', k, n);
    expect(planSummaryText(en, files)).toBe('6 files are blocked by CurseForge: 2 will come from Modrinth automatically, 4 you need to download yourself');
    expect(planSummaryText(de, files)).toContain('6 Dateien sind von CurseForge gesperrt: 2 kommen automatisch von Modrinth, 4 musst du selbst herunterladen');
    expect(planSummaryText(en, [file({})])).toBe('1 file is blocked by CurseForge: 1 you need to download yourself');
    expect(planSummaryText(en, [file({ state: 'matched_modrinth' })])).toBe('1 file is blocked by CurseForge: 1 will come from Modrinth automatically');
  });
});

describe('summaries and gating', () => {
  const files = [file({ fileName: 'a', state: 'pending' }), file({ fileName: 'b', state: 'found' }), file({ fileName: 'c', state: 'installed' }),
    file({ fileName: 'd', state: 'matched_modrinth' }), file({ fileName: 'e', state: 'skipped' })];
  it('counts found of total over the files the user has to provide', () => {
    expect(summarize(files)).toMatchObject({ total: 5, pending: 1, found: 1, installed: 1, matched: 1, skipped: 1, manual: 3, resolved: 2 });
    expect(summarize([])).toMatchObject({ manual: 0, resolved: 0 });
  });
  it('sorts pending first and lists only pending files as missing', () => {
    expect(sortBlocked(files).map((f) => f.state)).toEqual(['pending', 'found', 'installed', 'matched_modrinth', 'skipped']);
    expect(missingFiles(files).map((f) => f.fileName)).toEqual(['a']);
  });
  it('Done is silent only when nothing is pending', () => {
    expect(canFinishSilently(files)).toBe(false);
    expect(canFinishSilently(files.filter((f) => f.state !== 'pending'))).toBe(true);
    expect(canFinishSilently([])).toBe(true);
  });
  it('asks for confirmation above the threshold only', () => {
    expect(OPEN_ALL_CONFIRM_THRESHOLD).toBe(5);
    expect(openAllNeedsConfirm(5)).toBe(false);
    expect(openAllNeedsConfirm(6)).toBe(true);
  });
});

describe('open all missing pages', () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());
  it('opens one page at a time with a delay in between', async () => {
    const opened: string[] = [];
    const p = openPagesSequentially(['u1', 'u2', 'u3'], (u) => { opened.push(u); }, 1000);
    await vi.advanceTimersByTimeAsync(0);
    expect(opened).toEqual(['u1']);
    await vi.advanceTimersByTimeAsync(1000);
    expect(opened).toEqual(['u1', 'u2']);
    await vi.advanceTimersByTimeAsync(1000);
    expect(await p).toBe(3);
  });
  it('stops when cancelled', async () => {
    const opened: string[] = [];
    let cancel = false;
    const p = openPagesSequentially(['u1', 'u2', 'u3'], (u) => { opened.push(u); cancel = true; }, 1000, () => cancel);
    await vi.advanceTimersByTimeAsync(5000);
    expect(await p).toBe(1);
    expect(opened).toEqual(['u1']);
  });
});

describe('watch folder validation', () => {
  it('accepts absolute POSIX, drive and UNC paths only', () => {
    for (const ok of ['/home/me/Downloads', 'C:\\Users\\me\\Downloads', 'D:/dl', '\\\\nas\\share\\dl']) expect(isAbsolutePath(ok), ok).toBe(true);
    for (const bad of ['', '  ', 'Downloads', './dl', '~/Downloads', '/home/../etc', 'C:dl', 'a\0b', 'x'.repeat(2000)]) expect(isAbsolutePath(bad), bad).toBe(false);
  });
  it('rejects duplicates (case, trailing slash) and more than 8 folders', () => {
    expect(validateWatchDir(['/home/me/Downloads'], '/home/me/Downloads/')).toBe('duplicate');
    expect(validateWatchDir(['C:\\Dl'], 'c:\\dl')).toBe('duplicate');
    expect(validateWatchDir(['/a'], 'relative')).toBe('invalid');
    expect(validateWatchDir(['/a'], '/b')).toBeNull();
    expect(validateWatchDir(Array.from({ length: 8 }, (_, i) => `/d${i}`), '/new')).toBe('limit');
  });
  it('cleans trailing separators but keeps roots', () => {
    expect(cleanDir('  /home/me/Downloads/ ')).toBe('/home/me/Downloads');
    expect(cleanDir('/')).toBe('/');
    expect(cleanDir('C:\\')).toBe('C:\\');
  });
});

describe('i18n', () => {
  it('de and en cover the same blocked keys with the same placeholders', () => {
    expect(Object.keys(de6).sort()).toEqual(Object.keys(en6).sort());
    const vars = (s: string) => (s.match(/\{\w+\}/g) ?? []).sort().join(',');
    for (const k of Object.keys(en6) as (keyof typeof en6)[]) expect(vars(de6[k]), k).toBe(vars(en6[k]));
  });
});

describe('mock flow: install, watch, events, skip', () => {
  beforeEach(() => { vi.useFakeTimers(); resetStores(); });
  afterEach(() => vi.useRealTimers());

  async function installPack() {
    const api = createMockApi();
    await init(api);
    const project = { provider: 'curseforge' as const, projectId: 'x', title: 'Vault Hunters Reborn', type: 'modpack' as const, iconUrl: null };
    const search = await (async () => { const p = api.searchProjects({ params: { provider: 'curseforge', query: 'vault', projectType: 'modpack', sort: 'relevance', offset: 0, limit: 3 } }); await vi.advanceTimersByTimeAsync(400); return p; })();
    project.projectId = search.hits[0].projectId;
    const plan = api.modpackPlan({ provider: 'curseforge', projectId: project.projectId });
    await vi.advanceTimersByTimeAsync(400);
    const r = await startModpackRun(project, 'Vault', (await plan).versionId, true);
    await vi.advanceTimersByTimeAsync(3000);
    const p = await r.done;
    return { api, p: p! };
  }

  it('modpack_install seeds 4 pending + 2 Modrinth files, sets pendingBlocked and opens the blocked dialog', async () => {
    const { api, p } = await installPack();
    expect(p.pendingBlocked).toBe(4);
    expect(flow.req).toEqual({ kind: 'blocked', profileId: p.id });
    const files = await loadBlocked(p.id);
    expect(files.map((f) => f.state).sort()).toEqual(['matched_modrinth', 'matched_modrinth', 'pending', 'pending', 'pending', 'pending']);
    expect(app.profiles.find((x) => x.id === p.id)?.pendingBlocked).toBe(4);
    expect((await api.appState()).profiles.find((x) => x.id === p.id)?.pendingBlocked).toBe(4);
  });

  it('watcher lifecycle: nothing happens before start or after stop; a downloaded file goes found then installed via blocked:changed', async () => {
    const { api, p } = await installPack();
    await loadBlocked(p.id);
    const dev = api.dev;
    const name = dev.download(p.id) as string;
    await vi.advanceTimersByTimeAsync(5000);
    expect(blocked.files[p.id].find((f) => f.fileName === name)?.state).toBe('pending'); // not watching yet
    const stop = await startBlockedWatch(p.id);
    expect(dev.watching(p.id)).toBe(true);
    expect(blocked.watching[p.id]).toEqual(['/home/mango/Downloads']);
    await vi.advanceTimersByTimeAsync(1600);
    expect(blocked.files[p.id].find((f) => f.fileName === name)).toMatchObject({ state: 'found' });
    expect(blocked.files[p.id].find((f) => f.fileName === name)?.foundPath).toContain(name);
    await vi.advanceTimersByTimeAsync(700);
    expect(blocked.files[p.id].find((f) => f.fileName === name)?.state).toBe('installed');
    expect(app.profiles.find((x) => x.id === p.id)?.pendingBlocked).toBe(3);
    await stop(); await stop(); // idempotent
    expect(dev.watching(p.id)).toBe(false);
    expect(blocked.watching[p.id]).toBeUndefined();
    const second = dev.download(p.id) as string;
    await vi.advanceTimersByTimeAsync(5000);
    expect(blocked.files[p.id].find((f) => f.fileName === second)?.state).toBe('pending');
  });

  it('skip marks the file skipped and lowers the pending count; unknown files fail', async () => {
    const { p } = await installPack();
    const files = await loadBlocked(p.id);
    const pending = files.find((f) => f.state === 'pending') as BlockedFile;
    await skipBlocked(p.id, pending.fileName);
    expect(blocked.files[p.id].find((f) => f.fileName === pending.fileName)?.state).toBe('skipped');
    expect(app.profiles.find((x) => x.id === p.id)?.pendingBlocked).toBe(3);
    await expect(skipBlocked(p.id, 'nope.jar')).rejects.toMatchObject({ code: 'blocked_file_not_found' });
  });

  it('all downloaded files are found one by one every 1.5 s and the count reaches zero', async () => {
    const { api, p } = await installPack();
    await loadBlocked(p.id);
    const stop = await startBlockedWatch(p.id);
    const seen: number[] = [];
    for (let i = 0; i < 4; i++) { api.dev.download(p.id); await vi.advanceTimersByTimeAsync(1500); seen.push(blocked.files[p.id].filter((f) => f.state === 'found' || f.state === 'installed').length); }
    await vi.advanceTimersByTimeAsync(1000);
    expect(seen).toEqual([1, 2, 3, 4]);
    expect(app.profiles.find((x) => x.id === p.id)?.pendingBlocked).toBe(0);
    expect(summarize(blocked.files[p.id])).toMatchObject({ pending: 0, installed: 4, manual: 4, resolved: 4 });
    await stop();
  });
});
