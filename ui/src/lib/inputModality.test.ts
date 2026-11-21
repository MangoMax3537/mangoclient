// @vitest-environment jsdom
import { afterEach, describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import { installInputModality, isNavKey, modality } from './inputModality';

let off: (() => void) | null = null;
afterEach(() => { off?.(); off = null; });
const key = (k: string, o: KeyboardEventInit = {}) => document.dispatchEvent(new KeyboardEvent('keydown', { key: k, bubbles: true, ...o }));

describe('input modality', () => {
  it('starts as pointer, a navigation key switches to keyboard, a mouse press switches back', () => {
    off = installInputModality();
    expect(modality()).toBe('pointer');
    key('Tab');
    expect(modality()).toBe('keyboard');
    document.body.dispatchEvent(new MouseEvent('mousedown', { bubbles: true }));
    expect(modality()).toBe('pointer');
    key('ArrowDown');
    expect(modality()).toBe('keyboard');
  });
  it('typing text or a shortcut is not keyboard navigation', () => {
    off = installInputModality();
    key('a'); key('S', { ctrlKey: true }); key('Tab', { altKey: true });
    expect(modality()).toBe('pointer');
    expect(isNavKey({ key: 'Enter', ctrlKey: false, metaKey: false, altKey: false })).toBe(true);
  });
  it('the CSS hides every focus ring in pointer mode, and the app installs it', () => {
    const css = readFileSync(`${process.cwd()}/src/app.css`, 'utf8');
    expect(css).toMatch(/:root\[data-modality='pointer'\] :focus-visible[^{]*\{[^}]*outline: none !important/);
    expect(readFileSync(`${process.cwd()}/src/main.ts`, 'utf8')).toMatch(/installInputModality\(\)/);
  });
});
