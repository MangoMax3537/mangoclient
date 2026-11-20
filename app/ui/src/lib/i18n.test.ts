import { describe, expect, it } from 'vitest';
import { dictionaries, hasKey, plural, translate } from './i18n';

describe('i18n', () => {
  it('de and en have identical keys', () => {
    expect(Object.keys(dictionaries.de).sort()).toEqual(Object.keys(dictionaries.en).sort());
  });
  it('no empty strings; de differs from en for most keys (catches untranslated copies)', () => {
    const keys = Object.keys(dictionaries.en);
    for (const k of keys) {
      expect(dictionaries.en[k], k).toBeTruthy();
      expect(dictionaries.de[k], k).toBeTruthy();
    }
    const same = keys.filter((k) => dictionaries.de[k] === dictionaries.en[k]);
    expect(same.length / keys.length).toBeLessThan(0.2);
  });
  it('every {var} placeholder in en also appears in de', () => {
    const vars = (s: string) => (s.match(/\{\w+\}/g) ?? []).sort().join(',');
    for (const k of Object.keys(dictionaries.en)) expect(vars(dictionaries.de[k] as string), k).toBe(vars(dictionaries.en[k] as string));
  });
  it('every literal t()/tp() key used by the UI exists (no hard-coded or missing strings)', () => {
    const files = import.meta.glob('../**/*.svelte', { eager: true, query: '?raw', import: 'default' }) as Record<string, string>;
    expect(Object.keys(files).length).toBeGreaterThan(20);
    const missing: string[] = [];
    for (const [f, src] of Object.entries(files)) {
      for (const m of src.matchAll(/\bt(p?)\('([a-zA-Z0-9_.-]+)'/g)) {
        const k = m[2];
        const ok = m[1] ? hasKey('en', `${k}.other`) : hasKey('en', k);
        if (!ok) missing.push(`${f}: ${k}`);
      }
    }
    expect(missing).toEqual([]);
  });
  it('translates per language', () => {
    expect(translate('de', 'home.play')).toBe('Spielen');
    expect(translate('en', 'home.play')).toBe('Play');
  });
  it('falls back to English, then to the key', () => {
    dictionaries.de['test.only.en'] = undefined;
    dictionaries.en['test.only.en'] = 'English only';
    expect(translate('de', 'test.only.en')).toBe('English only');
    delete dictionaries.en['test.only.en'];
    expect(translate('de', 'test.only.en')).toBe('test.only.en');
  });
  it('interpolates {vars} and leaves unknown ones', () => {
    expect(translate('en', 'settings.ramOf', { total: '16 GB' })).toBe('of 16 GB installed');
    expect(translate('en', 'settings.ramOf')).toBe('of {total} installed');
  });
  it('plural picks one/other', () => {
    expect(plural('en', 'library.count', 1)).toBe('1 profile');
    expect(plural('en', 'library.count', 4)).toBe('4 profiles');
    expect(plural('de', 'library.count', 1)).toBe('1 Profil');
    expect(plural('de', 'library.count', 0)).toBe('0 Profile');
  });
});
