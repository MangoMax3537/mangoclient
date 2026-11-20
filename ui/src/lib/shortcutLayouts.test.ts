import { describe, expect, it } from 'vitest';
import { SHORTCUT_ALIASES, SHORTCUT_GROUPS, matchShortcut, type KeyLike } from './keybinds';
import { SETTINGS_INDEX } from './settingsIndex';
import { dictionaries } from './i18n';

const k = (key: string, code: string, o: Partial<KeyLike> = {}): KeyLike => ({ key, code, ctrlKey: false, metaKey: false, altKey: false, shiftKey: false, ...o });
const win = { mac: false, typing: false };

describe('shortcut sheet opens on every layout', () => {
  it('US: Ctrl+/, ?', () => {
    expect(matchShortcut(k('/', 'Slash', { ctrlKey: true }), win)).toBe('shortcuts');
    expect(matchShortcut(k('?', 'Slash', { shiftKey: true }), win)).toBe('shortcuts');
  });
  it('DE: Ctrl+Shift+7 (reports / or 7 with Digit7), Ctrl+Shift+ß (?), plain Shift+ß', () => {
    expect(matchShortcut(k('/', 'Digit7', { ctrlKey: true, shiftKey: true }), win)).toBe('shortcuts');
    expect(matchShortcut(k('7', 'Digit7', { ctrlKey: true, shiftKey: true }), win)).toBe('shortcuts');
    expect(matchShortcut(k('?', 'Minus', { ctrlKey: true, shiftKey: true }), win)).toBe('shortcuts');
    expect(matchShortcut(k('?', 'Minus', { shiftKey: true }), win)).toBe('shortcuts');
  });
  it('Cmd on macOS and F1 everywhere (also while typing)', () => {
    expect(matchShortcut(k('/', 'Slash', { metaKey: true }), { mac: true, typing: false })).toBe('shortcuts');
    expect(matchShortcut(k('F1', 'F1'), win)).toBe('shortcuts');
    expect(matchShortcut(k('F1', 'F1'), { mac: false, typing: true })).toBe('shortcuts');
  });
  it('plain "/" still focuses search, Ctrl+7 does nothing', () => {
    expect(matchShortcut(k('/', 'Slash'), win)).toBe('focusSearch');
    expect(matchShortcut(k('/', 'Digit7', { shiftKey: true }), win)).toBe('focusSearch');
    expect(matchShortcut(k('7', 'Digit7', { ctrlKey: true }), win)).toBeNull();
  });
  it('F2 renames (not while typing), F11 maximizes', () => {
    expect(matchShortcut(k('F2', 'F2'), win)).toBe('rename');
    expect(matchShortcut(k('F2', 'F2'), { mac: false, typing: true })).toBeNull();
    expect(matchShortcut(k('F11', 'F11'), win)).toBe('maximize');
  });
});

describe('one source of truth', () => {
  it('Settings index has one entry per shortcut, found by the shortcut aliases', () => {
    const n = SHORTCUT_GROUPS.reduce((a, g) => a + g.items.length, 0);
    const keyEntries = SETTINGS_INDEX.filter((e) => e.section === 'keys');
    expect(keyEntries.length).toBe(n);
    for (const a of ['tastenkürzel', 'shortcut', 'hotkey', 'keybinds', 'tastatur', 'keyboard']) {
      expect(SHORTCUT_ALIASES).toContain(a);
      expect(keyEntries.every((e) => e.aliases.includes(a))).toBe(true);
    }
  });
  it('groups are navigation, instances, search, window; none is empty; every label exists in de and en', () => {
    expect(SHORTCUT_GROUPS.map((g) => g.id)).toEqual(['navigate', 'instances', 'search', 'window']);
    for (const g of SHORTCUT_GROUPS) {
      expect(g.items.length).toBeGreaterThan(0);
      expect(dictionaries.de[g.title]).toBeTruthy();
      for (const it of g.items) { expect(dictionaries.de[it.label]).toBeTruthy(); expect(dictionaries.en[it.label]).toBeTruthy(); }
    }
  });
  it('key caps are platform-aware', () => {
    const find = (id: string) => SHORTCUT_GROUPS.flatMap((g) => g.items).find((i) => i.id === id)!;
    expect(find('home').keys(false)[0]).toEqual(['Ctrl', '1']);
    expect(find('home').keys(true)[0]).toEqual(['⌘', '1']);
    expect(find('back').keys(false)[0]).toEqual(['Alt', '←']);
    expect(find('playStop').keys(true)[0]).toEqual(['⌘', '⇧', 'P']);
  });
});
