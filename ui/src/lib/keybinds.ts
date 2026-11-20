// Global keyboard shortcuts as data + a pure matcher. `Mod` is Ctrl on Windows/Linux and Cmd on macOS. Nothing here touches the DOM
// except `isTypingTarget`; the actions are run by shortcutActions.ts. No shortcut collides with copy / paste / select all / undo.
import type { Key } from './i18n';

export type ShortcutAction =
  | 'home' | 'library' | 'discover' | 'servers' | 'skins' | 'stats' | 'settings'
  | 'newProfile' | 'focusSearch' | 'playStop' | 'back' | 'forward' | 'shortcuts' | 'rename' | 'maximize';

export interface KeyLike {
  key: string; code?: string;
  ctrlKey: boolean; metaKey: boolean; altKey: boolean; shiftKey: boolean;
  repeat?: boolean; isComposing?: boolean; defaultPrevented?: boolean;
}
export interface MatchContext {
  mac: boolean;
  /** Focus is in a text field, select or contenteditable. */
  typing: boolean;
  /** A modal dialog is open: only the shortcut sheet toggle still works. */
  modalOpen?: boolean;
}

const VIEW_BY_DIGIT: Record<string, ShortcutAction> = { '1': 'home', '2': 'library', '3': 'discover', '4': 'servers', '5': 'skins', '6': 'stats' };

/** The key text of a digit even when the layout needs Shift for it (French): code wins for Digit1..Digit6. */
function digitOf(e: KeyLike): string | null {
  if (e.code && /^Digit[1-6]$/.test(e.code)) return e.code.slice(5);
  return /^[1-6]$/.test(e.key) ? e.key : null;
}

/**
 * Is this the key that types `/` or `?` on the user's layout? Layout-independent: `key` is checked first (US `/`, `?`; German `Shift+7` yields `/`,
 * `Shift+ß` yields `?`), then the physical position: `Slash` (US) and `Shift+Digit7` (German, where the real shell reported neither `/` nor `?` with Ctrl held).
 */
export function isSlashKey(e: KeyLike): boolean {
  return e.key === '/' || e.key === '?' || e.code === 'Slash' || (e.code === 'Digit7' && e.shiftKey);
}

/**
 * Maps a key event to an action, or `null` (let the browser / the focused control handle it).
 * Rules: never while composing (IME) or when already handled or auto-repeating; in a text field only modifier combos fire
 * (and never plain keys such as `/` or `?`); with a modal dialog open only the shortcut sheet.
 */
export function matchShortcut(e: KeyLike, c: MatchContext): ShortcutAction | null {
  if (e.isComposing || e.defaultPrevented || e.repeat) return null;
  const mod = c.mac ? e.metaKey : e.ctrlKey;
  const otherMod = c.mac ? e.ctrlKey : e.metaKey;
  const k = e.key.length === 1 ? e.key.toLowerCase() : e.key;

  let a: ShortcutAction | null = null;
  const bare = !e.ctrlKey && !e.metaKey && !e.altKey && !e.shiftKey;
  if (bare && e.key === 'F1') a = 'shortcuts'; // layout-independent alternative; also fine while typing
  else if (bare && e.key === 'F11') a = 'maximize';
  else if (bare && e.key === 'F2' && !c.typing) a = 'rename';
  else if (mod && !otherMod && !e.altKey) {
    if (isSlashKey(e)) a = 'shortcuts'; // Ctrl+/ , Ctrl+Shift+7 (German), Ctrl+? (any layout)
    else if (!e.shiftKey) {
      const d = digitOf(e);
      if (d) a = VIEW_BY_DIGIT[d];
      else if (k === ',') a = 'settings';
      else if (k === 'n') a = 'newProfile';
      else if (k === 'f') a = 'focusSearch';
      else if (c.mac && k === '[') a = 'back';
      else if (c.mac && k === ']') a = 'forward';
    } else if (k === 'p') a = 'playStop';
  } else if (!mod && !otherMod) {
    if (e.altKey && !e.shiftKey && !c.mac) {
      if (e.key === 'ArrowLeft') a = 'back';
      else if (e.key === 'ArrowRight') a = 'forward';
    } else if (!e.altKey && !c.typing) {
      if (e.key === '?' || (e.code === 'Slash' && e.shiftKey)) a = 'shortcuts';
      else if (e.key === '/' || e.code === 'Slash' || (e.code === 'Digit7' && e.shiftKey)) a = 'focusSearch'; // `/` needs Shift on some layouts (German)
    }
  }
  if (a === null) return null;
  if (c.modalOpen && a !== 'shortcuts') return null;
  return a;
}

/** Mouse side buttons: 3 = back, 4 = forward (MouseEvent.button). */
export const mouseNav = (button: number): 'back' | 'forward' | null => (button === 3 ? 'back' : button === 4 ? 'forward' : null);

export function isTypingTarget(t: EventTarget | null): boolean {
  const el = t as (Element & { isContentEditable?: boolean }) | null;
  if (!el || typeof el.tagName !== 'string') return false;
  const tag = el.tagName.toLowerCase();
  if (tag === 'textarea' || tag === 'select') return true;
  if (tag === 'input') {
    const type = (el.getAttribute('type') ?? 'text').toLowerCase();
    return !['button', 'checkbox', 'radio', 'range', 'submit', 'reset', 'color', 'file', 'image'].includes(type);
  }
  return !!el.isContentEditable;
}

/* ---- the sheet (also the Settings section): ONE data source ---- */
export interface ShortcutItem {
  id: string;
  /** The action the global listener runs; `null` for keys a component handles itself (Enter on a card, the Menu key). */
  action: ShortcutAction | null;
  label: Key;
  /** One or more alternative combinations, each an array of key caps. */
  keys: (mac: boolean) => string[][];
  /** Extra words the settings search matches (de + en, not shown). */
  aliases?: readonly string[];
}
export interface ShortcutGroup { id: 'navigate' | 'instances' | 'search' | 'window'; title: Key; items: ShortcutItem[] }

const MOD = (mac: boolean): string => (mac ? '⌘' : 'Ctrl');
const ALT = (mac: boolean): string => (mac ? '⌥' : 'Alt');
const SHIFT = (mac: boolean): string => (mac ? '⇧' : 'Shift');

/** Every shortcut, grouped. Key caps are platform-aware: Ctrl vs the Command key, Alt vs the Option key. */
export const SHORTCUT_GROUPS: readonly ShortcutGroup[] = [
  { id: 'navigate', title: 'keys.group.navigate', items: [
    { id: 'home', action: 'home', label: 'nav.home', keys: (m) => [[MOD(m), '1']] },
    { id: 'library', action: 'library', label: 'nav.library', keys: (m) => [[MOD(m), '2']] },
    { id: 'discover', action: 'discover', label: 'nav.discover', keys: (m) => [[MOD(m), '3']] },
    { id: 'servers', action: 'servers', label: 'nav.servers', keys: (m) => [[MOD(m), '4']] },
    { id: 'skins', action: 'skins', label: 'nav.skins', keys: (m) => [[MOD(m), '5']] },
    { id: 'stats', action: 'stats', label: 'nav.stats', keys: (m) => [[MOD(m), '6']] },
    { id: 'settings', action: 'settings', label: 'nav.settings', keys: (m) => [[MOD(m), ',']] },
    { id: 'back', action: 'back', label: 'keys.back', keys: (m) => (m ? [[MOD(m), '[']] : [[ALT(m), '←']]) },
    { id: 'forward', action: 'forward', label: 'keys.forward', keys: (m) => (m ? [[MOD(m), ']']] : [[ALT(m), '→']]) },
  ] },
  { id: 'instances', title: 'keys.group.instances', items: [
    { id: 'newProfile', action: 'newProfile', label: 'library.new', keys: (m) => [[MOD(m), 'N']] },
    { id: 'playStop', action: 'playStop', label: 'keys.playStop', keys: (m) => [[MOD(m), SHIFT(m), 'P']] },
    { id: 'rename', action: 'rename', label: 'keys.rename', keys: () => [['F2']] },
    { id: 'openCard', action: null, label: 'keys.openCard', keys: () => [['Enter']] },
    { id: 'contextMenu', action: null, label: 'keys.contextMenu', keys: (m) => [[SHIFT(m), 'F10'], ['Menu']], aliases: ['kontextmenü', 'rechtsklick', 'right click', 'context menu'] },
  ] },
  { id: 'search', title: 'keys.group.search', items: [
    { id: 'focusSearch', action: 'focusSearch', label: 'keys.search', keys: (m) => [[MOD(m), 'F'], ['/']] },
    { id: 'shortcuts', action: 'shortcuts', label: 'keys.sheet', keys: (m) => [['?'], [MOD(m), '/'], ['F1']] },
    { id: 'close', action: null, label: 'keys.close', keys: () => [['Esc']] },
  ] },
  { id: 'window', title: 'keys.group.window', items: [
    { id: 'maximize', action: 'maximize', label: 'keys.maximize', keys: () => [['F11']], aliases: ['vollbild', 'fullscreen', 'maximieren', 'fenster', 'window'] },
  ] },
];

/** The words every shortcut entry is found by in the settings search. */
export const SHORTCUT_ALIASES: readonly string[] = ['tastenkürzel', 'tastenkombination', 'shortcut', 'shortcuts', 'hotkey', 'hotkeys', 'keybinds', 'keybind', 'tastatur', 'keyboard', 'kürzel', 'tasten', 'keys'];

/** `Ctrl+1` / `⌘1` for the tooltip of a control; the first combination of the action. */
export function shortcutLabel(action: ShortcutAction, mac: boolean): string {
  for (const g of SHORTCUT_GROUPS) for (const it of g.items) if (it.action === action) {
    const first = it.keys(mac)[0];
    return mac ? first.join('') : first.join('+');
  }
  return '';
}
/** A combination as text for screen readers and search: `Ctrl+Shift+P`, `⌘⇧P`. */
export const comboText = (combo: readonly string[], mac: boolean): string => (mac ? combo.join('') : combo.join('+'));
