import { describe, expect, it } from 'vitest';
import { dictionaries, plural, translate } from './i18n';

describe('i18n', () => {
  it('de and en have identical keys', () => {
    expect(Object.keys(dictionaries.de).sort()).toEqual(Object.keys(dictionaries.en).sort());
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
