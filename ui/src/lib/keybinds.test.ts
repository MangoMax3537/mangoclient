import { describe, expect, it } from 'vitest';
import { SHORTCUT_GROUPS, capText, isTypingTarget, matchShortcut, mouseNav, shortcutLabel, type KeyLike } from './keybinds';
import { translate } from './i18n';

const k = (key: string, o: Partial<KeyLike> = {}): KeyLike => ({ key, ctrlKey: false, metaKey: false, altKey: false, shiftKey: false, ...o });
const win = { mac: false, typing: false };
const mac = { mac: true, typing: false };

describe('matchShortcut', () => {
  it('Mod+1..6 switch views, Ctrl on Windows/Linux and Cmd on macOS', () => {
    const views = ['home', 'library', 'discover', 'servers', 'skins', 'stats'];
    views.forEach((v, i) => {
      expect(matchShortcut(k(String(i + 1), { ctrlKey: true }), win)).toBe(v);
      expect(matchShortcut(k(String(i + 1), { metaKey: true }), mac)).toBe(v);
    });
    expect(matchShortcut(k('1', { metaKey: true }), win)).toBeNull();
    expect(matchShortcut(k('1', { ctrlKey: true }), mac)).toBeNull();
    expect(matchShortcut(k('7', { ctrlKey: true }), win)).toBeNull();
    expect(matchShortcut({ ...k('!', { ctrlKey: true, shiftKey: true }), code: 'Digit1' }, win)).toBeNull();
  });
  it('settings, new profile, search, sheet, play/stop', () => {
    expect(matchShortcut(k(',', { ctrlKey: true }), win)).toBe('settings');
    expect(matchShortcut(k('n', { ctrlKey: true }), win)).toBe('newProfile');
    expect(matchShortcut(k('f', { ctrlKey: true }), win)).toBe('focusSearch');
    expect(matchShortcut(k('/'), win)).toBe('focusSearch');
    expect(matchShortcut(k('?', { shiftKey: true }), win)).toBe('shortcuts');
    expect(matchShortcut(k('/', { ctrlKey: true }), win)).toBe('shortcuts');
    expect(matchShortcut(k('P', { ctrlKey: true, shiftKey: true }), win)).toBe('playStop');
    expect(matchShortcut(k('p', { ctrlKey: true }), win)).toBeNull();
  });
  it('back / forward: Alt+arrows, Cmd+[ and Cmd+] on macOS', () => {
    expect(matchShortcut(k('ArrowLeft', { altKey: true }), win)).toBe('back');
    expect(matchShortcut(k('ArrowRight', { altKey: true }), win)).toBe('forward');
    expect(matchShortcut(k('[', { metaKey: true }), mac)).toBe('back');
    expect(matchShortcut(k(']', { metaKey: true }), mac)).toBe('forward');
    expect(matchShortcut(k('[', { ctrlKey: true }), win)).toBeNull();
    expect(matchShortcut(k('ArrowLeft', { altKey: true }), mac)).toBeNull();
    expect(matchShortcut(k('ArrowLeft'), win)).toBeNull();
  });
  it('never takes copy / paste / select all / undo / cut / find-next', () => {
    for (const key of ['c', 'v', 'a', 'z', 'x', 'y', 's', 'r', 'w', 'q']) {
      expect(matchShortcut(k(key, { ctrlKey: true }), win), key).toBeNull();
      expect(matchShortcut(k(key, { metaKey: true }), mac), key).toBeNull();
    }
  });
  it('input guard: plain keys do not fire while typing, modifier combos still do', () => {
    const typing = { mac: false, typing: true };
    expect(matchShortcut(k('/'), typing)).toBeNull();
    expect(matchShortcut(k('?', { shiftKey: true }), typing)).toBeNull();
    expect(matchShortcut(k('2', { ctrlKey: true }), typing)).toBe('library');
    expect(matchShortcut(k('f', { ctrlKey: true }), typing)).toBe('focusSearch');
    expect(matchShortcut(k('ArrowLeft', { altKey: true }), typing)).toBe('back');
  });
  it('a modal dialog blocks everything but the sheet', () => {
    expect(matchShortcut(k('2', { ctrlKey: true }), { ...win, modalOpen: true })).toBeNull();
    expect(matchShortcut(k('/', { ctrlKey: true }), { ...win, modalOpen: true })).toBe('shortcuts');
  });
  it('ignores IME composition, handled events and auto-repeat', () => {
    expect(matchShortcut(k('2', { ctrlKey: true, isComposing: true }), win)).toBeNull();
    expect(matchShortcut(k('2', { ctrlKey: true, defaultPrevented: true }), win)).toBeNull();
    expect(matchShortcut(k('2', { ctrlKey: true, repeat: true }), win)).toBeNull();
  });
  it('mouse side buttons', () => {
    expect(mouseNav(3)).toBe('back'); expect(mouseNav(4)).toBe('forward'); expect(mouseNav(0)).toBeNull(); expect(mouseNav(1)).toBeNull();
  });
});

describe('isTypingTarget', () => {
  const el = (tagName: string, type?: string, ce = false) => ({ tagName, isContentEditable: ce, getAttribute: () => type ?? null }) as unknown as EventTarget;
  it('text fields yes, buttons and checkboxes no', () => {
    expect(isTypingTarget(el('INPUT', 'text'))).toBe(true);
    expect(isTypingTarget(el('INPUT'))).toBe(true);
    expect(isTypingTarget(el('INPUT', 'search'))).toBe(true);
    expect(isTypingTarget(el('TEXTAREA'))).toBe(true);
    expect(isTypingTarget(el('SELECT'))).toBe(true);
    expect(isTypingTarget(el('DIV', undefined, true))).toBe(true);
    expect(isTypingTarget(el('INPUT', 'checkbox'))).toBe(false);
    expect(isTypingTarget(el('BUTTON'))).toBe(false);
    expect(isTypingTarget(null)).toBe(false);
  });
});

describe('shortcut sheet content', () => {
  const flat = SHORTCUT_GROUPS.flatMap((g) => g.items);
  it('lists every action the matcher can return', () => {
    const actions = new Set(flat.map((i) => i.action));
    for (const a of ['home', 'library', 'discover', 'servers', 'skins', 'stats', 'settings', 'newProfile', 'focusSearch', 'playStop', 'back', 'forward', 'shortcuts']) expect(actions.has(a as never), a).toBe(true);
  });
  it('key labels are platform aware', () => {
    const home = flat.find((i) => i.id === 'home')!;
    expect(home.keys(false)).toEqual([['Ctrl', '1']]);
    expect(home.keys(true)).toEqual([['⌘', '1']]);
    expect(flat.find((i) => i.id === 'back')!.keys(false)).toEqual([['Alt', '←']]);
    expect(flat.find((i) => i.id === 'back')!.keys(true)).toEqual([['⌘', '[']]);
    expect(flat.find((i) => i.id === 'playStop')!.keys(true)).toEqual([['⌘', '⇧', 'P']]);
    expect(shortcutLabel('skins', false)).toBe('Ctrl+5');
    expect(shortcutLabel('skins', true)).toBe('⌘5');
  });
  it('every label and group title is translated in both languages', () => {
    for (const g of SHORTCUT_GROUPS) {
      expect(translate('de', g.title)).not.toBe(g.title); expect(translate('en', g.title)).not.toBe(g.title);
      for (const i of g.items) { expect(translate('de', i.label)).not.toBe(i.label); expect(translate('en', i.label)).not.toBe(i.label); }
    }
  });
});

describe('key caps follow the UI language', () => {
  it('German keyboards say Strg, Umschalt and Eingabe; English and Mac symbols stay', () => {
    expect(capText('Ctrl', 'de')).toBe('Strg');
    expect(capText('Shift', 'de')).toBe('Umschalt');
    expect(capText('Enter', 'de')).toBe('Eingabe');
    expect(capText('F2', 'de')).toBe('F2');
    expect(capText('Ctrl', 'en')).toBe('Ctrl');
    expect(capText('⌘', 'de')).toBe('⌘');
  });
  it('the tooltip label is localized too', () => {
    expect(shortcutLabel('skins', false, 'de')).toBe('Strg+5');
    expect(shortcutLabel('skins', false, 'en')).toBe('Ctrl+5');
    expect(shortcutLabel('playStop', false, 'de')).toBe('Strg+Umschalt+P');
  });
});
