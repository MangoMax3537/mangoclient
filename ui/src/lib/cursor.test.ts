// Pointer-cursor audit: the base CSS must give every clickable element type a pointer, disabled ones not-allowed, and no component may fight it
// with `cursor: default`. A DOM pass over the shared components' markup checks that every element with an onclick is covered by the central selector.
import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';

const css = readFileSync(new URL('../app.css', import.meta.url), 'utf8');

const files = import.meta.glob('../**/*.svelte', { eager: true, query: '?raw', import: 'default' }) as Record<string, string>;
const pointerRule = css.split('\n').find((l) => l.includes('cursor: pointer;') && l.startsWith(':where(')) ?? '';

describe('cursors', () => {
  it('base css covers the interactive roles and elements', () => {
    for (const s of ['button', "[role='tab']", "[role='menuitem']", "[role='switch']", 'a[href]', 'label', 'select', "input[type='checkbox']", "input[type='radio']", "input[type='range']", 'summary', '.clickable']) expect(pointerRule, s).toContain(s);
    expect(css).toMatch(/not-allowed/);
    expect(css).toMatch(/cursor: text/);
  });
  it('no component forces cursor: default', () => {
    const bad = Object.entries(files).filter(([, s]) => /cursor:\s*default/.test(s)).map(([f]) => f);
    expect(bad).toEqual([]);
    expect(css).not.toMatch(/cursor:\s*default/);
  });
  it('every element with a click handler in the components is a button, link, role or .clickable', () => {
    const offenders: string[] = [];
    for (const [f, src] of Object.entries(files)) {
      const markup = src.replace(/<script[\s\S]*?<\/script>/g, '').replace(/<style[\s\S]*?<\/style>/g, '');
      for (const m of markup.matchAll(/<(div|span|li|p|section|article|img|svg|canvas)\b([^>]*?)\sonclick=/g)) {
        if (/role=|class="[^"]*clickable/.test(m[2])) continue;
        offenders.push(`${f}: <${m[1]}${m[2].slice(0, 40)}`);
      }
    }
    expect(offenders).toEqual([]);
  });
  it('the 3D viewer is grab / grabbing and the resize zones carry resize cursors', async () => {
    const skin = (await import('./skin3d?raw')).default as string;
    expect(skin).toContain("'grabbing'"); expect(skin).toContain("'grab'");
    const { RESIZE_ZONES } = await import('./titlebar');
    expect(RESIZE_ZONES.every((z) => /resize$/.test(z.cursor))).toBe(true);
  });
});
