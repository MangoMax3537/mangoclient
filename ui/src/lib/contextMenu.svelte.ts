// The one context menu of the app: a single store (so exactly one menu is open at a time), pure placement / keyboard helpers (unit-tested),
// the `use:contextmenu` action for right-click, `openMenuFor` for a three-dots button, and the global guard that suppresses the native
// webview menu everywhere except inside text fields and on selected text. The menu itself is components/ContextMenu.svelte (mounted once).
import type { Component } from 'svelte';
import { isTypingTarget } from './keybinds';

export interface MenuItem {
  id: string;
  label?: string;
  /** A Phosphor icon component. */
  icon?: Component<any>; // eslint-disable-line @typescript-eslint/no-explicit-any
  /** Hint on the right ("F2", "Ctrl+Shift+P"). */
  shortcut?: string;
  danger?: boolean;
  disabled?: boolean;
  /** Tooltip of a disabled item: say WHY it is unavailable. A disabled item without a reason should not exist; leave it out. */
  reason?: string;
  run?: () => void;
  separator?: boolean;
  /** Dropped from the list (keeps call sites flat). */
  hidden?: boolean;
}
export const SEP: MenuItem = { id: '-', separator: true };

/** Drops hidden items, then leading / trailing / doubled separators. */
export function tidy(items: readonly MenuItem[]): MenuItem[] {
  const out: MenuItem[] = [];
  for (const it of items) {
    if (it.hidden) continue;
    if (it.separator && (out.length === 0 || out[out.length - 1].separator)) continue;
    out.push(it);
  }
  while (out.length && out[out.length - 1].separator) out.pop();
  return out;
}

export interface Size { w: number; h: number }
export interface Place { left: number; top: number; flippedX: boolean; flippedY: boolean }
export type At = { x: number; y: number } | { rect: { left: number; right: number; top: number; bottom: number } };
const MARGIN = 8;

/**
 * Where the menu goes. A pointer position opens with the corner at the pointer and flips left / up at the viewport edge; a button rect opens
 * below the button, right edges aligned, and flips above it when there is no room. Always clamped inside the viewport.
 */
export function placeMenu(at: At, size: Size, vp: Size): Place {
  const clampX = (x: number) => Math.max(MARGIN, Math.min(x, vp.w - size.w - MARGIN));
  const clampY = (y: number) => Math.max(MARGIN, Math.min(y, vp.h - size.h - MARGIN));
  if ('x' in at) {
    const flippedX = at.x + size.w > vp.w - MARGIN;
    const flippedY = at.y + size.h > vp.h - MARGIN;
    return { left: clampX(flippedX ? at.x - size.w : at.x), top: clampY(flippedY ? at.y - size.h : at.y), flippedX, flippedY };
  }
  const r = at.rect;
  const flippedX = r.right - size.w < MARGIN;
  const flippedY = r.bottom + 4 + size.h > vp.h - MARGIN;
  return { left: clampX(flippedX ? r.left : r.right - size.w), top: clampY(flippedY ? r.top - 4 - size.h : r.bottom + 4), flippedX, flippedY };
}

/** Index of the next focusable (non-separator) item in `dir` direction, wrapping; `-1` start means "before the first". */
export function stepIndex(items: readonly MenuItem[], from: number, dir: 1 | -1): number {
  const n = items.length;
  if (!n) return -1;
  let i = from;
  for (let k = 0; k < n; k++) {
    i = i < 0 ? (dir === 1 ? 0 : n - 1) : (i + dir + n) % n;
    if (!items[i].separator) return i;
  }
  return -1;
}
export const edgeIndex = (items: readonly MenuItem[], end: 'first' | 'last'): number => {
  const idx = items.map((it, i) => (it.separator ? -1 : i)).filter((i) => i >= 0);
  return idx.length ? (end === 'first' ? idx[0] : idx[idx.length - 1]) : -1;
};
/** Type-ahead: the next item (after `from`, wrapping) whose label starts with `buffer`. */
export function typeaheadIndex(items: readonly MenuItem[], buffer: string, from: number): number {
  const b = buffer.toLowerCase();
  if (!b) return -1;
  const n = items.length;
  for (let k = 1; k <= n; k++) {
    const i = (from + k + n) % n;
    const it = items[i];
    if (!it.separator && (it.label ?? '').toLowerCase().startsWith(b)) return i;
  }
  return -1;
}

/* ---- store ---- */
export const cm = $state({
  open: false,
  items: [] as MenuItem[],
  at: { x: 0, y: 0 } as At,
  /** The element that opened it (a button: it gets aria-expanded and focus back). */
  anchor: null as HTMLElement | null,
  /** Opened by keyboard: the first item is focused right away. */
  keyboard: false,
  label: '',
  /** Bumped on every open so the component re-places and re-focuses even for the same items. */
  seq: 0,
});
let returnTo: HTMLElement | null = null;

export function openMenu(items: readonly MenuItem[], at: At, o: { anchor?: HTMLElement | null; keyboard?: boolean; label?: string } = {}): void {
  const list = tidy(items);
  if (!list.length) { closeMenu(false); return; }
  returnTo = o.anchor ?? (document.activeElement instanceof HTMLElement ? document.activeElement : null);
  cm.items = list; cm.at = at; cm.anchor = o.anchor ?? null; cm.keyboard = !!o.keyboard; cm.label = o.label ?? ''; cm.open = true; cm.seq++;
}
export function closeMenu(restoreFocus = true): void {
  if (!cm.open) return;
  cm.open = false; cm.anchor = null;
  const el = returnTo; returnTo = null;
  if (restoreFocus && el && el.isConnected) el.focus({ preventScroll: true });
}

/** Opens the menu below a button (the three-dots trigger); a second click on the open button closes it. */
export function openMenuFor(btn: HTMLElement, items: readonly MenuItem[], label = ''): void {
  if (cm.open && cm.anchor === btn) { closeMenu(); return; }
  openMenu(items, { rect: btn.getBoundingClientRect() }, { anchor: btn, keyboard: false, label });
}

/* ---- native menu policy ---- */
/** Native copy / paste stays in text fields and on selected text; everywhere else the webview menu is suppressed. */
export function keepsNativeMenu(target: EventTarget | null, sel: Selection | null = typeof getSelection === 'function' ? getSelection() : null): boolean {
  const el = target as Element | null;
  if (!el || typeof el.closest !== 'function') return false;
  if (isTypingTarget(el) || el.closest('textarea, input:not([type="button"],[type="checkbox"],[type="radio"],[type="range"],[type="submit"]), [contenteditable]:not([contenteditable="false"])')) return true;
  if (sel && !sel.isCollapsed && sel.toString().trim() !== '') {
    try { if (sel.rangeCount && sel.getRangeAt(0).intersectsNode(el)) return true; } catch { /* detached */ }
    if (sel.anchorNode && el.contains(sel.anchorNode)) return true;
  }
  return false;
}

/** Window-level guard: an element action that handled the event has already called preventDefault; everything else is cancelled unless native is wanted. */
export function installNativeMenuGuard(doc: Document = document): () => void {
  const on = (e: MouseEvent) => { if (!e.defaultPrevented && !keepsNativeMenu(e.target, doc.getSelection())) e.preventDefault(); };
  doc.addEventListener('contextmenu', on);
  return () => doc.removeEventListener('contextmenu', on);
}

/** `use:contextmenu={() => items}`: right-click (and the keyboard Menu key / Shift+F10, which also fire `contextmenu`) opens the menu at the pointer. */
export function contextmenu(node: HTMLElement, getItems: (() => readonly MenuItem[]) | readonly MenuItem[]) {
  let cur = getItems;
  const on = (e: MouseEvent) => {
    if (e.defaultPrevented || keepsNativeMenu(e.target)) return;
    e.preventDefault();
    const items = typeof cur === 'function' ? cur() : cur;
    const fromKeyboard = e.clientX === 0 && e.clientY === 0 && e.button === 0;
    if (fromKeyboard) openMenu(items, { rect: node.getBoundingClientRect() }, { anchor: node, keyboard: true });
    else openMenu(items, { x: e.clientX, y: e.clientY }, { anchor: null });
  };
  node.addEventListener('contextmenu', on);
  return { update(g: typeof getItems) { cur = g; }, destroy() { node.removeEventListener('contextmenu', on); } };
}
