// Selection outlines and focus rings must not be clipped by a scroll container: grids inside one have inner padding and no negative margin.
import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
const read = (n: string): string => readFileSync(fileURLToPath(new URL(`../components/${n}`, import.meta.url)), 'utf8');

describe('picker grids', () => {
  for (const f of ['BackgroundPicker.svelte', 'IconPicker.svelte', 'InstancePicker.svelte', 'Skins.svelte']) {
    it(`${f}: no negative margins in the style block`, () => {
      const css = read(f).split('<style>')[1] ?? '';
      expect(css.replace(/margin-top: -2px/g, '')).not.toMatch(/margin(-[a-z]+)?:\s*-\d/);
    });
  }
  it('the background grid scrolls vertically with a defined height and 6px inner padding', () => {
    const g = (read('BackgroundPicker.svelte').match(/\n\s*\.g \{[^}]*\}/) ?? [''])[0];
    expect(g).toMatch(/max-height:\s*\d+px/);
    expect(g).toMatch(/overflow-y:\s*auto/);
    expect(g).toMatch(/padding:\s*6px/);
    // without max-content rows the tracks shrink to fit max-height (tiles overlap / rows are cut): the real cause of the clipped grid
    expect(g).toMatch(/grid-auto-rows:\s*max-content/);
  });
  it('a grid item never combines aspect-ratio with overflow:hidden (its track collapses to the caption height and the rows overlap)', () => {
    const th = (read('BackgroundPicker.svelte').match(/\n\s*\.th \{[^}]*\}/) ?? [''])[0];
    expect(th).toMatch(/overflow:\s*hidden/);
    expect(th).not.toMatch(/aspect-ratio/);
  });
  it('the Mango Client tile comes first, before the image wallpapers', () => {
    const s = read('BackgroundPicker.svelte');
    expect(s.indexOf('MANGO_CLIENT_ID, label')).toBeGreaterThan(0);
    expect(s.indexOf('MANGO_CLIENT_ID, label')).toBeLessThan(s.indexOf('{#each all as w'));
  });
});
