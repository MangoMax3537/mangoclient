import { describe, expect, it } from 'vitest';
import {
  analyzable, causeView, checkGroup, checkTag, classifyDrop, CRASH_CODES, defaultDiagnosticsName, defaultExportName, exportBlocker, exportErrorKey, filterProfiles,
  groupSections, importDefaultName, importErrorKey, importReduce, importRetryable, initialImport, isImportFile, knownGroups, knownTags, normalizeTags, rankCauses, reportText,
  safeFileStem, withExtension, type ImportState,
} from './portability';
import { dictionaries, hasKey, translate } from './i18n';
import type { CrashCause, CrashReport, ImportPlan } from './types';

const plan: ImportPlan = { kind: 'mrpack', name: 'Pack', mcVersion: '1.21.1', loader: 'fabric', loaderVersion: '0.16.10', fileCount: 3, sizeBytes: 1000, blockedFiles: [], ram: null, appliedRamMb: null };

describe('tags and group validation (mirrors the service)', () => {
  it('trims, rejects empty, control characters, long values', () => {
    expect(checkTag('  sodium ', [])).toEqual({ ok: true, tag: 'sodium' });
    expect(checkTag('   ', [])).toEqual({ ok: false, error: 'empty' });
    expect(checkTag('a\u0007b', [])).toEqual({ ok: false, error: 'control' });
    expect(checkTag('x'.repeat(24), []).ok).toBe(true);
    expect(checkTag('x'.repeat(25), [])).toEqual({ ok: false, error: 'long' });
  });
  it('rejects case-insensitive duplicates and more than 12 tags', () => {
    expect(checkTag('Create', ['create'])).toEqual({ ok: false, error: 'duplicate' });
    const twelve = Array.from({ length: 12 }, (_, i) => `t${i}`);
    expect(checkTag('one-more', twelve)).toEqual({ ok: false, error: 'limit' });
    expect(checkTag('t3', twelve)).toEqual({ ok: false, error: 'duplicate' });
  });
  it('normalizeTags keeps the first spelling, drops invalid ones and caps at 12', () => {
    expect(normalizeTags(['Create', ' create ', '', 'Tech', 'x'.repeat(30)])).toEqual(['Create', 'Tech']);
    expect(normalizeTags(Array.from({ length: 20 }, (_, i) => `t${i}`))).toHaveLength(12);
  });
  it('group: blank means none, 48 characters at most', () => {
    expect(checkGroup('  ')).toEqual({ ok: true, group: null });
    expect(checkGroup(' Modpacks ')).toEqual({ ok: true, group: 'Modpacks' });
    expect(checkGroup('g'.repeat(48)).ok).toBe(true);
    expect(checkGroup('g'.repeat(49))).toEqual({ ok: false, error: 'long' });
    expect(checkGroup('a\nb')).toEqual({ ok: false, error: 'control' });
  });
  it('counts characters, not UTF-16 units', () => {
    expect(checkTag('\u{1F34B}'.repeat(24), []).ok).toBe(true);
    expect(checkTag('\u{1F34B}'.repeat(25), []).ok).toBe(false);
  });
  it('knownGroups / knownTags', () => {
    const l = [{ group: 'B', tags: ['x', 'Y'] }, { group: 'a', tags: ['y'] }, { group: 'b', tags: [] }, { group: null, tags: ['x'] }];
    expect(knownGroups(l)).toEqual(['a', 'B']);
    expect(knownTags(l)).toEqual([{ tag: 'x', count: 2 }, { tag: 'Y', count: 2 }]);
  });
});

describe('library sections and search', () => {
  const p = (name: string, group: string | null, tags: string[] = [], loader = 'fabric', mc = '1.21.1') => ({ name, group, tags, loader: loader as 'fabric', mcVersion: mc });
  const list = [p('Iron', 'Survival', ['main']), p('Vanilla', 'survival'), p('Bed', 'PvP', ['1.8']), p('Solo', null, ['shaders']), p('Pack', 'Modpacks')];
  it('groups are alphabetical, case-insensitive, ungrouped last', () => {
    const s = groupSections(list);
    expect(s.map((x) => x.group)).toEqual(['Modpacks', 'PvP', 'Survival', null]);
    expect(s[2].items.map((x) => x.name)).toEqual(['Iron', 'Vanilla']);
    expect(s[3].key).toBe('ungrouped');
  });
  it('without groups there is a single ungrouped section', () => {
    const s = groupSections([p('a', null), p('b', null)]);
    expect(s).toHaveLength(1);
    expect(s[0].group).toBeNull();
  });
  it('an empty list has no sections', () => { expect(groupSections([])).toEqual([]); });
  it('searches name, group, tags, version and loader', () => {
    expect(filterProfiles(list, 'surv').map((x) => x.name)).toEqual(['Iron', 'Vanilla']);
    expect(filterProfiles(list, 'shad').map((x) => x.name)).toEqual(['Solo']);
    expect(filterProfiles(list, '1.8').map((x) => x.name)).toEqual(['Bed']);
    expect(filterProfiles(list, 'iron').map((x) => x.name)).toEqual(['Iron']);
  });
  it('tag filter is exact and case-insensitive and combines with the query', () => {
    expect(filterProfiles(list, '', 'MAIN').map((x) => x.name)).toEqual(['Iron']);
    expect(filterProfiles(list, 'van', 'main')).toEqual([]);
  });
});

describe('drop handling', () => {
  it('recognises modpack files by extension', () => {
    expect(isImportFile('/d/a.mrpack')).toBe(true);
    expect(isImportFile('C:\\d\\A.ZIP')).toBe(true);
    expect(isImportFile('/d/a.png')).toBe(false);
    expect(isImportFile('/d/zip')).toBe(false);
  });
  it('opens the first pack, counts extras and ignored files', () => {
    expect(classifyDrop(['/a.png', '/b.mrpack', '/c.zip', '/d.txt'])).toEqual({ file: '/b.mrpack', extra: 1, ignored: 2 });
    expect(classifyDrop(['/a.png'])).toEqual({ file: null, extra: 0, ignored: 1 });
    expect(classifyDrop([])).toEqual({ file: null, extra: 0, ignored: 0 });
  });
});

describe('import state machine', () => {
  const run = (st: ImportState, ...ev: Parameters<typeof importReduce>[1][]) => ev.reduce(importReduce, st);
  it('plan -> rename -> start -> imported', () => {
    let st = run(initialImport('/d/Pack.mrpack'), { e: 'planned', plan });
    expect(st).toMatchObject({ s: 'plan', name: 'Pack' });
    st = run(st, { e: 'rename', name: 'My pack' }, { e: 'start' });
    expect(st).toMatchObject({ s: 'importing', name: 'My pack' });
    expect(run(st, { e: 'imported', profileId: 'x' })).toEqual({ s: 'done', profileId: 'x' });
  });
  it('an empty or overlong name cannot start', () => {
    const planned = run(initialImport('/p.mrpack'), { e: 'planned', plan });
    expect(run(planned, { e: 'rename', name: '  ' }, { e: 'start' }).s).toBe('plan');
    expect(run(planned, { e: 'rename', name: 'n'.repeat(49) }, { e: 'start' }).s).toBe('plan');
  });
  it('a failed plan is a dead end, a failed import can go back to the plan with the edited name', () => {
    const failedPlan = run(initialImport('/p.mrpack'), { e: 'failed', code: 'path.not_granted' });
    expect(failedPlan).toMatchObject({ s: 'error', during: 'plan', code: 'path.not_granted' });
    expect(run(failedPlan, { e: 'back' })).toBe(failedPlan);
    const failedImport = run(initialImport('/p.mrpack'), { e: 'planned', plan }, { e: 'rename', name: 'Mine' }, { e: 'start' }, { e: 'failed', code: 'net.request' });
    expect(failedImport).toMatchObject({ s: 'error', during: 'import' });
    expect(run(failedImport, { e: 'back' })).toMatchObject({ s: 'plan', name: 'Mine' });
  });
  it('ignores events that do not fit the state', () => {
    const st = initialImport('/p.mrpack');
    expect(run(st, { e: 'start' })).toBe(st);
    expect(run(st, { e: 'imported', profileId: 'x' })).toBe(st);
    expect(run(st, { e: 'rename', name: 'x' })).toBe(st);
  });
  it('default name falls back to the file stem', () => {
    expect(importDefaultName({ ...plan, name: ' ' }, '/d/My Pack.mrpack')).toBe('My Pack');
  });
});

describe('error mapping', () => {
  it('maps the documented codes to messages that exist in both languages', () => {
    const codes = ['path.not_granted', 'path.unsupported', 'net.host_not_allowed', 'loaders.installer_unverified', 'import.not_found', 'import.not_a_file', 'import.too_large',
      'import.not_an_archive', 'import.unsupported_format', 'import.invalid_pack', 'import.unsafe_archive', 'curseforge.no_api_key'];
    for (const c of codes) {
      const k = importErrorKey(c);
      expect(k, c).not.toBeNull();
      expect(hasKey('en', k as string) && hasKey('de', k as string), c).toBe(true);
    }
    for (const c of ['export.invalid_path', 'export.lookup_failed', 'export.loader_unresolved', 'export.too_large', 'export.failed', 'path.not_granted']) {
      expect(hasKey('en', exportErrorKey(c) as string), c).toBe(true);
    }
    expect(importErrorKey('something.else')).toBeNull();
  });
  it('path.not_granted tells the user to choose the file again with the Import button', () => {
    expect(translate('en', importErrorKey('path.not_granted') as string)).toMatch(/choose the file again with the Import button/i);
    expect(importRetryable('path.not_granted')).toBe(true);
    expect(importRetryable('import.too_large')).toBe(false);
  });
});

describe('export and diagnostics options', () => {
  it('default file names are safe', () => {
    expect(defaultExportName('Iron Hollow')).toBe('Iron Hollow.mrpack');
    expect(defaultExportName('a/b:c*d')).toBe('a_b_c_d.mrpack');
    expect(defaultExportName('...')).toBe('instance.mrpack');
    expect(defaultDiagnosticsName('Iron Hollow')).toBe('mangoclient-diagnostics-Iron-Hollow.zip');
    expect(safeFileStem('x'.repeat(200))).toHaveLength(80);
  });
  it('the export button is dark with a reason until a usable absolute path exists', () => {
    expect(exportBlocker({ includeConfig: false, outPath: '' })).toBe('noPath');
    expect(exportBlocker({ includeConfig: true, outPath: 'relative/x.mrpack' })).toBe('badPath');
    expect(exportBlocker({ includeConfig: false, outPath: '/home/u/x.mrpack' })).toBeNull();
    expect(exportBlocker({ includeConfig: false, outPath: 'C:\\Users\\u\\x.mrpack' })).toBeNull();
  });
  it('adds the extension once', () => {
    expect(withExtension('/a/b', '.mrpack')).toBe('/a/b.mrpack');
    expect(withExtension('/a/b.MRPACK', '.mrpack')).toBe('/a/b.MRPACK');
  });
});

const cause = (code: string, confidence: CrashCause['confidence'], extra: Partial<CrashCause> = {}): CrashCause =>
  ({ code, title: `T ${code}`, detail: `D ${code}`, confidence, suggestions: ['s'], relatedMods: [], ...extra });

describe('crash panel mapping', () => {
  it('ranks by confidence and keeps the service order inside a level', () => {
    const r = rankCauses([cause('a', 'low'), cause('b', 'high'), cause('c', 'medium'), cause('d', 'high')]);
    expect(r.map((x) => x.code)).toEqual(['b', 'd', 'c', 'a']);
  });
  it('every documented rule code is localized with a title, a detail and two tips in both languages', () => {
    for (const c of CRASH_CODES) for (const lang of ['en', 'de'] as const) for (const part of ['title', 'detail', 's1', 's2']) {
      expect(dictionaries[lang][`crash.code.${c}.${part}`], `${lang} ${c} ${part}`).toBeTruthy();
    }
  });
  it('English shows the service text, German the localized text plus the original as a quote; unknown codes fall back', () => {
    const c = cause('out_of_memory', 'high', { detail: 'Java heap space (1024 MB)' });
    const tr = (lang: 'en' | 'de') => (k: string) => translate(lang, k);
    const en = causeView(c, 'en', tr('en') as never, (k) => hasKey('en', k));
    expect(en).toMatchObject({ title: 'T out_of_memory', detail: 'Java heap space (1024 MB)', fromLog: null });
    const de = causeView(c, 'de', tr('de') as never, (k) => hasKey('de', k));
    expect(de.title).toBe('Dem Spiel ging der Speicher aus');
    expect(de.fromLog).toBe('Java heap space (1024 MB)');
    expect(de.suggestions).toHaveLength(2);
    const unknown = causeView(cause('brand_new', 'low'), 'de', tr('de') as never, (k) => hasKey('de', k));
    expect(unknown).toMatchObject({ title: 'T brand_new', detail: 'D brand_new', fromLog: null });
  });
  it('the report text lists causes in rank order with tips, mods and the excerpt', () => {
    const rep: CrashReport = { summary: 'Boom', causes: [cause('x', 'low'), cause('y', 'high', { relatedMods: ['Sodium'] })], javaVersion: '21', mcVersion: '1.21.4', excerpt: 'line1\n', sourceLog: 'crash:c.txt' };
    const t = reportText(rep, 'Iron');
    expect(t.indexOf('T y')).toBeLessThan(t.indexOf('T x'));
    expect(t).toContain('Mods: Sodium');
    expect(t).toContain('Java: 21');
    expect(t).toMatch(/Excerpt:\nline1$/);
  });
  it('analyze is offered for crash reports and the latest log only', () => {
    expect(analyzable({ kind: 'crash', id: 'crash:c.txt' })).toBe(true);
    expect(analyzable({ kind: 'game', id: 'game:latest.log' })).toBe(true);
    expect(analyzable({ kind: 'game', id: 'latest.log' })).toBe(true);
    expect(analyzable({ kind: 'game', id: 'game:2026-11-15-1.log' })).toBe(false);
    expect(analyzable({ kind: 'launcher', id: 'launcher.log' })).toBe(false);
  });
});
