// @vitest-environment jsdom
import { beforeEach, describe, expect, it } from 'vitest';
import { cm, closeMenu, contextmenu, edgeIndex, installNativeMenuGuard, keepsNativeMenu, openMenu, openMenuFor, placeMenu, stepIndex, tidy, typeaheadIndex, SEP, type MenuItem } from './contextMenu.svelte';

const it3: MenuItem[] = [{ id: 'a', label: 'Alpha' }, SEP, { id: 'b', label: 'Beta' }, { id: 'c', label: 'Bravo' }];
const vp = { w: 1000, h: 700 };

describe('placeMenu', () => {
  it('opens at the pointer', () => {
    expect(placeMenu({ x: 100, y: 120 }, { w: 200, h: 300 }, vp)).toMatchObject({ left: 100, top: 120, flippedX: false, flippedY: false });
  });
  it('flips left and up at the viewport edges, never leaves the viewport', () => {
    const p = placeMenu({ x: 950, y: 650 }, { w: 200, h: 300 }, vp);
    expect(p).toMatchObject({ left: 750, top: 350, flippedX: true, flippedY: true });
    const q = placeMenu({ x: 5, y: 5 }, { w: 2000, h: 2000 }, vp);
    expect(q.left).toBe(8); expect(q.top).toBe(8);
  });
  it('anchors below a button, right edges aligned, flips above when there is no room', () => {
    const r = { left: 700, right: 740, top: 100, bottom: 130 };
    expect(placeMenu({ rect: r }, { w: 200, h: 200 }, vp)).toMatchObject({ left: 540, top: 134, flippedY: false });
    const low = { left: 700, right: 740, top: 600, bottom: 630 };
    expect(placeMenu({ rect: low }, { w: 200, h: 200 }, vp)).toMatchObject({ top: 396, flippedY: true });
    expect(placeMenu({ rect: { left: 10, right: 40, top: 10, bottom: 40 } }, { w: 200, h: 100 }, vp).left).toBe(10);
  });
});

describe('keyboard helpers', () => {
  it('skips separators, wraps, Home / End', () => {
    expect(stepIndex(it3, 0, 1)).toBe(2);
    expect(stepIndex(it3, 3, 1)).toBe(0);
    expect(stepIndex(it3, 0, -1)).toBe(3);
    expect(stepIndex(it3, -1, 1)).toBe(0);
    expect(edgeIndex(it3, 'first')).toBe(0);
    expect(edgeIndex(it3, 'last')).toBe(3);
  });
  it('type-ahead finds the next label with the prefix, wrapping', () => {
    expect(typeaheadIndex(it3, 'b', 0)).toBe(2);
    expect(typeaheadIndex(it3, 'b', 2)).toBe(3);
    expect(typeaheadIndex(it3, 'br', 0)).toBe(3);
    expect(typeaheadIndex(it3, 'z', 0)).toBe(-1);
  });
  it('tidy drops hidden items and stray separators', () => {
    expect(tidy([SEP, { id: 'x', hidden: true }, { id: 'a', label: 'A' }, SEP, SEP, { id: 'b', hidden: true }, SEP]).map((i) => i.id)).toEqual(['a']);
  });
});

describe('store: one menu at a time', () => {
  beforeEach(() => closeMenu(false));
  it('opening a second menu replaces the first; empty menus do not open', () => {
    openMenu([{ id: 'a', label: 'A' }], { x: 1, y: 1 });
    const s1 = cm.seq;
    openMenu([{ id: 'b', label: 'B' }], { x: 2, y: 2 });
    expect(cm.open).toBe(true); expect(cm.items.map((i) => i.id)).toEqual(['b']); expect(cm.seq).toBeGreaterThan(s1);
    openMenu([{ id: 'h', hidden: true }], { x: 1, y: 1 });
    expect(cm.open).toBe(false);
  });
  it('a button toggles its own menu and returns focus on close', () => {
    const b = document.createElement('button'); document.body.appendChild(b);
    openMenuFor(b, [{ id: 'a', label: 'A' }]);
    expect(cm.open).toBe(true); expect(cm.anchor).toBe(b);
    openMenuFor(b, [{ id: 'a', label: 'A' }]);
    expect(cm.open).toBe(false); expect(document.activeElement).toBe(b);
    b.remove();
  });
});

describe('native menu policy', () => {
  beforeEach(() => { getSelection()?.removeAllRanges(); });
  it('stays native in text fields and contenteditable, suppressed elsewhere', () => {
    const input = document.createElement('input'); const ta = document.createElement('textarea'); const div = document.createElement('div'); const btn = document.createElement('button');
    const ce = document.createElement('div'); ce.setAttribute('contenteditable', 'true');
    for (const el of [input, ta, ce]) document.body.appendChild(el);
    document.body.append(div, btn);
    expect(keepsNativeMenu(input, null)).toBe(true);
    expect(keepsNativeMenu(ta, null)).toBe(true);
    expect(keepsNativeMenu(ce, null)).toBe(true);
    expect(keepsNativeMenu(div, null)).toBe(false);
    expect(keepsNativeMenu(btn, null)).toBe(false);
  });
  it('stays native on selected text', () => {
    const p = document.createElement('p'); p.textContent = 'hello world'; document.body.appendChild(p);
    const sel = getSelection(); sel?.selectAllChildren(p);
    expect(keepsNativeMenu(p, sel)).toBe(true);
    sel?.removeAllRanges();
    expect(keepsNativeMenu(p, getSelection())).toBe(false);
  });
  it('the global guard cancels the event unless native is wanted or an action handled it', () => {
    const off = installNativeMenuGuard(document);
    const div = document.createElement('div'); const input = document.createElement('input'); document.body.append(div, input);
    const fire = (el: Element) => { const e = new MouseEvent('contextmenu', { bubbles: true, cancelable: true }); el.dispatchEvent(e); return e.defaultPrevented; };
    expect(fire(div)).toBe(true);
    expect(fire(input)).toBe(false);
    off();
  });
  it('use:contextmenu opens at the pointer and skips inputs; empty areas stay menu-free', () => {
    closeMenu(false);
    const host = document.createElement('div'); const inner = document.createElement('input'); host.appendChild(inner); document.body.appendChild(host);
    const a = contextmenu(host, () => [{ id: 'x', label: 'X' }]);
    inner.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX: 10, clientY: 20 }));
    expect(cm.open).toBe(false);
    host.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX: 10, clientY: 20 }));
    expect(cm.open).toBe(true); expect(cm.at).toEqual({ x: 10, y: 20 });
    a.destroy(); closeMenu(false);
  });
});
