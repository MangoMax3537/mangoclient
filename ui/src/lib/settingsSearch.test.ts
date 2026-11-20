import { describe, expect, it } from 'vitest';
import { dictionaries, translate, type Lang } from './i18n';
import { SECTIONS, SETTINGS_INDEX, entriesFor, sectionOf, type SettingEntry } from './settingsIndex';
import { editDistance, fold, keyStep, normalize, searchSettings, tokenTier, wordsOf } from './settingsSearch';

const resolver = (lang: Lang) => ({
  label: (e: SettingEntry) => translate(lang, e.label),
  desc: (e: SettingEntry) => (e.desc ? translate(lang, e.desc) : ''),
  section: (e: SettingEntry) => translate(lang, sectionOf(e).label),
});
const app = entriesFor('app');
const inst = entriesFor('instance');
const ids = (q: string, lang: Lang = 'en', entries: readonly SettingEntry[] = app) => searchSettings(entries, q, resolver(lang)).map((h) => h.entry.id);

describe('normalization', () => {
  it('lowercases, drops diacritics and expands umlauts and ß', () => {
    expect(normalize('Aufräumen')).toBe('aufraeumen');
    expect(normalize('Größe')).toBe('groesse');
    expect(normalize('STRASSE')).toBe('strasse');
    expect(normalize('Straße')).toBe('strasse');
    expect(normalize('Café')).toBe('cafe');
    expect(fold('Aufräumen')).toBe('aufraumen');
    expect(fold('Größe')).toBe('grosse');
  });
  it('words come in both spellings, so "aufraumen" and "aufräumen" find the same thing', () => {
    expect(wordsOf('Aufräumen')).toEqual(expect.arrayContaining(['aufraeumen', 'aufraumen']));
    expect(ids('aufraumen', 'de')).toContain('storage');
    expect(ids('aufräumen', 'de')).toContain('storage');
    expect(ids('aufraeumen', 'de')).toContain('storage');
  });
  it('typo distance counts one swap or one edit as 1', () => {
    expect(editDistance('darkmode', 'darkmoed')).toBe(1);
    expect(editDistance('speicher', 'speichr')).toBe(1);
    expect(editDistance('ram', 'rom')).toBe(1);
    expect(editDistance('abcdef', 'abcxyz', 1)).toBe(3);
  });
  it('typo tolerance only applies from 5 letters', () => {
    expect(tokenTier('darkmoed', 'darkmode')).toBeGreaterThan(0);
    expect(tokenTier('rom', 'ram')).toBe(0);
    expect(tokenTier('exact', 'exact')).toBe(1);
    expect(tokenTier('pre', 'prefix')).toBe(0.9);
    expect(tokenTier('efix', 'prefix')).toBe(0.6);
    expect(tokenTier('fix', 'prefix')).toBe(0); // too short to match inside a word
    expect(tokenTier('fi', 'prefix')).toBe(0);
  });
});

describe('alias hits', () => {
  it('darkmode, dark mode, dunkel, appearance and farbschema all lead to the theme setting', () => {
    for (const q of ['darkmode', 'dark mode', 'dunkel', 'Appearance', 'farbschema', 'DARKMODE', 'Darstellung', 'light', 'hell']) expect(ids(q)[0], q).toBe('theme');
  });
  it('reports which alias matched', () => {
    const hit = searchSettings(app, 'darkmode', resolver('en'))[0];
    expect(hit).toMatchObject({ via: 'alias', alias: 'darkmode' });
    const label = searchSettings(app, 'theme', resolver('en'))[0];
    expect(label.entry.id).toBe('theme');
  });
  it('the documented synonyms resolve to the documented settings', () => {
    const expected: Record<string, string> = {
      sprache: 'language', locale: 'language', deutsch: 'language', xmx: 'ram', heap: 'ram', arbeitsspeicher: 'ram', flags: 'javaArgs', jvm: 'javaArgs',
      vollbild: 'fullscreen', auflösung: 'windowSize', resolution: 'windowSize', telemetry: 'telemetry', analytics: 'telemetry', datenschutz: 'telemetry', privacy: 'telemetry',
      aktualisieren: 'updates', release: 'updates', festplatte: 'storage', aufräumen: 'storage', disk: 'storage', sicherung: 'autoSnapshot', snapshot: 'autoSnapshot',
      konto: 'accounts', login: 'accounts', microsoft: 'accounts',
    };
    for (const [q, id] of Object.entries(expected)) expect(ids(q, 'de').slice(0, 2), q).toContain(id);
  });
  it('instance settings: background, icon, group, loader, delete', () => {
    const expected: Record<string, string> = {
      wallpaper: 'background', hintergrund: 'background', panorama: 'background', symbol: 'icon', block: 'icon', modpack: 'icon', gruppe: 'grouptags', tags: 'grouptags',
      fabric: 'version', forge: 'version', neoforge: 'version', minecraft: 'version', löschen: 'delete', entfernen: 'delete', ram: 'ram', mangoconfig: 'mango',
    };
    for (const [q, id] of Object.entries(expected)) expect(ids(q, 'de', inst).slice(0, 2), q).toContain(id);
  });
});

describe('ranking', () => {
  it('"ram" leads to the memory setting first', () => {
    expect(ids('ram')[0]).toBe('ram');
    expect(ids('ram', 'de')).toEqual(['ram']); // not "parameter" (the alias of the Java arguments)
  });
  it('"speicher" ranks both memory and storage, near the top, in a stable sensible order', () => {
    const de = ids('speicher', 'de');
    expect(de.slice(0, 2).sort()).toEqual(['ram', 'storage']);
    // German label "Speicher" is the storage section; the RAM row matches through its alias, so storage ranks first
    expect(de[0]).toBe('storage');
    const en = ids('speicher', 'en');
    expect(en.slice(0, 2).sort()).toEqual(['ram', 'storage']);
  });
  it('label matches outrank alias matches, which outrank description-only matches', () => {
    const hits = searchSettings(app, 'backup', resolver('en'));
    expect(['autoSnapshot', 'backupKeep']).toContain(hits[0].entry.id);
    const labelWins = searchSettings(app, 'language', resolver('en'));
    expect(labelWins[0]).toMatchObject({ via: 'label' });
    // "usage" only appears in a description (telemetry hint: "usage ping" is in its label, "install id" in the hint)
    const desc = searchSettings(app, 'install', resolver('en')).find((h) => h.entry.id === 'telemetry');
    expect(desc?.via).toBe('desc');
    expect(desc!.score).toBeLessThan(searchSettings(app, 'telemetry', resolver('en'))[0].score);
  });
  it('every token must match: "dark ram" finds nothing, "dark mode" finds the theme', () => {
    expect(ids('dark ram')).toEqual([]);
    expect(ids('dark mode')).toEqual(['theme']);
  });
  it('an empty or whitespace query finds nothing', () => {
    expect(ids('')).toEqual([]);
    expect(ids('   ')).toEqual([]);
  });
});

describe('typo tolerance', () => {
  it('one typo in a word of 5+ letters still finds it', () => {
    expect(ids('darkmoed')[0]).toBe('theme');
    expect(ids('arbeitsspeicer', 'de')[0]).toBe('ram');
    expect(ids('sprahce', 'de')[0]).toBe('language');
    expect(ids('telemtry')[0]).toBe('telemetry');
  });
  it('short words and two typos do not match', () => {
    expect(ids('rom')).toEqual([]);
    expect(ids('darkmxxe')).toEqual([]);
    expect(ids('qqqqqq')).toEqual([]);
  });
});

describe('index integrity', () => {
  it('every entry has a section, an existing label and description key in both languages and unique ids per scope', () => {
    for (const e of SETTINGS_INDEX) {
      expect(SECTIONS.some((s) => s.id === e.section && s.scope === e.scope), e.id).toBe(true);
      for (const key of [e.label, ...(e.desc ? [e.desc] : []), sectionOf(e).label]) {
        expect(dictionaries.en[key], `${e.id}:${key}`).toBeTruthy();
        expect(dictionaries.de[key], `${e.id}:${key}`).toBeTruthy();
      }
      expect(e.target).toBe(`set-${e.id}`);
      expect(e.aliases.length, e.id).toBeGreaterThan(0);
    }
    for (const scope of ['app', 'instance'] as const) {
      const list = entriesFor(scope).map((e) => e.id);
      expect(new Set(list).size).toBe(list.length);
    }
  });
  it('the search UI strings exist in both languages', () => {
    for (const k of ['settings.search.label', 'settings.search.placeholder', 'settings.search.none', 'settings.search.matched', 'settings.search.clear']) {
      expect(dictionaries.en[k]).toBeTruthy(); expect(dictionaries.de[k]).toBeTruthy(); expect(dictionaries.de[k]).not.toBe(dictionaries.en[k]);
    }
    expect(translate('en', 'settings.search.none', { q: 'xyz' })).toBe('Nothing found for "xyz"');
    expect(translate('en', 'settings.search.matched', { alias: 'darkmode' })).toBe('matched: darkmode');
  });
});

describe('keyboard flow', () => {
  it('arrows move with wrap-around, Home/End jump, Enter picks the active result, Escape clears', () => {
    expect(keyStep('ArrowDown', 0, 3)).toEqual({ active: 1, action: 'none' });
    expect(keyStep('ArrowDown', 2, 3)).toEqual({ active: 0, action: 'none' });
    expect(keyStep('ArrowUp', 0, 3)).toEqual({ active: 2, action: 'none' });
    expect(keyStep('End', 0, 3)).toEqual({ active: 2, action: 'none' });
    expect(keyStep('Home', 2, 3)).toEqual({ active: 0, action: 'none' });
    expect(keyStep('Enter', 1, 3)).toEqual({ active: 1, action: 'pick' });
    expect(keyStep('Enter', 7, 3)).toEqual({ active: 2, action: 'pick' });
    expect(keyStep('Escape', 2, 3)).toEqual({ active: 0, action: 'clear' });
    expect(keyStep('x', 1, 3)).toEqual({ active: 1, action: 'none' });
  });
  it('with no results Enter and arrows do nothing, Escape still clears', () => {
    expect(keyStep('Enter', 0, 0).action).toBe('none');
    expect(keyStep('ArrowDown', 0, 0).action).toBe('none');
    expect(keyStep('Escape', 0, 0).action).toBe('clear');
  });
  it('typing, arrowing and Enter reach the right entry', () => {
    const hits = searchSettings(app, 'backup', resolver('en'));
    expect(hits.length).toBeGreaterThanOrEqual(2);
    let active = 0;
    let picked: string | null = null;
    for (const key of ['ArrowDown', 'Enter']) {
      const step = keyStep(key, active, hits.length);
      active = step.active;
      if (step.action === 'pick') picked = hits[active].entry.target;
    }
    expect(picked).toBe(hits[1].entry.target);
  });
});
