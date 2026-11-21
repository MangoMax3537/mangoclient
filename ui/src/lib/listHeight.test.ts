import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';

const src = (p: string) => readFileSync(new URL(`../components/${p}`, import.meta.url), 'utf8');

// WebKitGTK (the real app's engine) does not resolve `height: 100%` against a flexed parent; with `contain: strict` the content list then
// collapsed to 0 px and the Mods tab showed an EMPTY list although the tab said "Mods 3". Chromium/Firefox never showed it, so guard the CSS.
describe('content list can never collapse to nothing', () => {
  it('VirtualList has no size containment and fills flex parents by flex-basis', () => {
    const css = src('VirtualList.svelte');
    expect(css).not.toMatch(/contain:\s*strict/);
    expect(css).toMatch(/flex:\s*1 1 0/);
  });
  it('ContentTab has a floor height and does not depend on height: 100%', () => {
    const css = src('profile/ContentTab.svelte');
    expect(css).toMatch(/\.ct \{[^}]*min-height: 320px/);
    expect(css).not.toMatch(/\.ct \{[^}]*height: 100%/);
    expect(css).toMatch(/\.list \{[^}]*min-height: 200px/);
  });
  it('the Overview mods tile takes the same count as the tab', () => {
    expect(src('ProfilePage.svelte')).toContain('modCount={items ? counts.mod : p.modCount}');
    expect(src('profile/Overview.svelte')).not.toContain('profile.modCount');
  });
});
