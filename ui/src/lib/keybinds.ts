// Global keyboard shortcuts as data + a pure matcher. `Mod` is Ctrl on Windows/Linux and Cmd on macOS. Nothing here touches the DOM
// except `isTypingTarget`; the actions are run by shortcutActions.ts. No shortcut collides with copy / paste / select all / undo.
import type { Key } from './i18n';

export type ShortcutAction =
  | 'home' | 'library' | 'discover' | 'servers' | 'skins' | 'stats' | 'settings'
  | 'newProfile' | 'focusSearch' | 'playStop' | 'back' | 'forward' | 'shortcuts';

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
  if (mod && !otherMod && !e.altKey) {
    if (!e.shiftKey) {
      const d = digitOf(e);
      if (d) a = VIEW_BY_DIGIT[d];
      else if (k === ',') a = 'settings';
      else if (k === 'n') a = 'newProfile';
      else if (k === 'f') a = 'focusSearch';
      else if (k === '/') a = 'shortcuts';
      else if (c.mac && k === '[') a = 'back';
      else if (c.mac && k === ']') a = 'forward';
    } else if (k === 'p') a = 'playStop';
    else if (k === '/') a = 'shortcuts'; // `/` needs Shift on some layouts (German)
  } else if (!mod && !otherMod) {
    if (e.altKey && !e.shiftKey && !c.mac) {
      if (e.key === 'ArrowLeft') a = 'back';
      else if (e.key === 'ArrowRight') a = 'forward';
    } else if (!e.altKey && !c.typing) {
      if (e.key === '?') a = 'shortcuts';
      else if (e.key === '/') a = 'focusSearch'; // `/` needs Shift on some layouts (German), so Shift is not checked
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

/* ---- the sheet ---- */
export interface ShortcutDef { action: ShortcutAction; label: Key; /** extra keys shown for the same action */ alt?: boolean }
export interface ShortcutGroup { title: Key; items: { id: string; action: ShortcutAction; label: Key; keys: (mac: boolean) => string[][] }[] }

const MOD = (mac: boolean): string => (mac ? '⌘' : 'Ctrl');
const ALT = (mac: boolean): string => (mac ? '⌥' : 'Alt');
const SHIFT = (mac: boolean): string => (mac ? '⇧' : 'Shift');

/** Every shortcut, grouped; each entry lists one or more key combinations (alternatives) as arrays of key caps. */
export const SHORTCUT_GROUPS: readonly ShortcutGroup[] = [
  { title: 'keys.group.navigate', items: [
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
  { title: 'keys.group.actions', items: [
    { id: 'newProfile', action: 'newProfile', label: 'library.new', keys: (m) => [[MOD(m), 'N']] },
    { id: 'focusSearch', action: 'focusSearch', label: 'keys.search', keys: (m) => [[MOD(m), 'F'], ['/']] },
    { id: 'playStop', action: 'playStop', label: 'keys.playStop', keys: (m) => [[MOD(m), SHIFT(m), 'P']] },
  ] },
  { title: 'keys.group.general', items: [
    { id: 'close', action: 'shortcuts', label: 'keys.close', keys: () => [['Esc']] },
    { id: 'shortcuts', action: 'shortcuts', label: 'keys.sheet', keys: (m) => [['?'], [MOD(m), '/']] },
  ] },
];

/** `Ctrl+1` / `⌘1` for the tooltip of a control; the first combination of the action. */
export function shortcutLabel(action: ShortcutAction, mac: boolean): string {
  for (const g of SHORTCUT_GROUPS) for (const it of g.items) if (it.action === action && it.id !== 'close') {
    const first = it.keys(mac)[0];
    return mac ? first.join('') : first.join('+');
  }
  return '';
}
