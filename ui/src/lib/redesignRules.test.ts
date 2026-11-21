import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { activeSection, showNavigator, NAV_MIN_WIDTH } from './scrollspy';
import { engineProfile, flatten, FULL_BLUR, FULL_DIM } from './bgMode';
import { entriesFor } from './settingsIndex';
import { de15, en15 } from './i18n15';

const src = (f: string) => readFileSync(fileURLToPath(new URL(f, import.meta.url)), 'utf8');

describe('scrollspy', () => {
  const tops = (...t: number[]) => t.map((top, i) => ({ id: `s${i}`, top }));
  it('the section nearest the top (last one past the line) is active', () => {
    expect(activeSection(tops(10, 400, 900))).toBe('s0');
    expect(activeSection(tops(-300, 60, 500))).toBe('s1');
    expect(activeSection(tops(-900, -400, 80))).toBe('s2');
  });
  it('before the first section: the first; at the bottom: the last; nothing: null', () => {
    expect(activeSection(tops(300, 700))).toBe('s0');
    expect(activeSection(tops(-300, 200, 400), { atBottom: true })).toBe('s2');
    expect(activeSection([])).toBeNull();
  });
  it('the navigator needs two or more sections', () => {
    expect(showNavigator([])).toBe(false); expect(showNavigator([1])).toBe(false); expect(showNavigator([1, 2])).toBe(true);
  });
});

describe('settings navigator', () => {
  it('is hidden below 1100 px, the content column stays centred', () => {
    expect(NAV_MIN_WIDTH).toBe(1100);
    const css = src('../components/SettingsLayout.svelte');
    expect(css).toMatch(/\.side \{ display: none; \}/);
    expect(css).toMatch(/@media \(min-width: 1100px\)/);
    expect(css).toMatch(/minmax\(0, 1fr\) minmax\(0, var\(--w\)\) minmax\(0, 1fr\)/);
  });
  it('lists the real sections: Settings and the instance tab derive the items from sec(), so search matches filter them', () => {
    expect(src('../components/Settings.svelte')).toMatch(/\.filter\(\(\[id\]\) => sec\(id\)\)/);
    expect(src('../components/profile/ProfileSettings.svelte')).toMatch(/\.filter\(\(\[id\]\) => sec\(id\.slice\(2\)\)\)/);
    // every listed section id exists in the search index
    const ids = new Set(entriesFor('app').map((e) => e.section));
    for (const id of ['look', 'launcher', 'accounts', 'game', 'backups', 'keys', 'storage', 'updates']) expect(ids.has(id), id).toBe(true);
  });
  it('is a landmark with a label, marks the active item and is keyboard operable (real buttons)', () => {
    const s = src('../components/SettingsNav.svelte');
    expect(s).toMatch(/<nav [^>]*aria-label=/); expect(s).toMatch(/aria-current=\{active === it\.id \? 'true' : undefined\}/);
    expect(s).toMatch(/<button type="button"/); expect(s).toMatch(/prefers-reduced-motion: reduce/); expect(s).toMatch(/h\.focus\(\{ preventScroll: true \}\)/);
  });
  it('de and en carry the same new keys', () => {
    expect(Object.keys(de15).sort()).toEqual(Object.keys(en15).sort());
  });
});

describe('background everywhere', () => {
  it('flattens the panorama cube to its front face for the full-window layer, uses a strong scrim', () => {
    const set = { mcVersion: '1.21', faces: ['f0', 'f1', 'f2', 'f3', 'f4', 'f5'] } as never;
    expect(flatten({ kind: 'panorama', set })).toMatchObject({ kind: 'image', url: 'f0' });
    expect(flatten({ kind: 'plain' })).toEqual({ kind: 'plain' });
    expect(FULL_DIM).toBeGreaterThanOrEqual(0.75); expect(FULL_BLUR).toBe(10);
  });
  it('the engine follows the open instance page, else the selected instance, else nothing', () => {
    const a = { id: 'a' }, b = { id: 'b' };
    expect(engineProfile('profile', 'b', [a, b], a)).toBe(b);
    expect(engineProfile('library', null, [a, b], a)).toBe(a);
    expect(engineProfile('home', null, [], null)).toBeNull();
  });
  it('one engine in the shell, none in the views', () => {
    expect(src('../App.svelte')).toMatch(/<AppBackground /);
    for (const f of ['Home', 'Library', 'Servers', 'Skins', 'Stats', 'Settings', 'Discover']) expect(src(`../components/${f}.svelte`), f).not.toMatch(/AppBackground|PanoramaBackground|BgLayer/);
  });
  it('the Settings toggle exists with search aliases', () => {
    expect(src('../components/Settings.svelte')).toMatch(/setBackgroundEverywhere/);
    expect(entriesFor('app').find((e) => e.id === 'bgAll')?.aliases).toContain('everywhere');
  });
});

describe('neutral panels, no white rings, no rail rings', () => {
  it('instance card info panels are the grey surface in every state; selection is a 1px accent frame', () => {
    const lib = src('../components/Library.svelte');
    expect(lib).not.toMatch(/\.info \{[^}]*tint/); expect(lib).not.toMatch(/\.card\.sel \.info/); expect(lib).not.toMatch(/color-mix\(in srgb, var\(--accent-fill\)/);
    expect(lib).toMatch(/\.info \{ background: var\(--ink-2\); \}/);
    // drawn by the shared .frame ring (follows the notched corners), see notchGuard.test.ts
    expect(lib).toMatch(/\.card\.sel \{ --frame: var\(--accent-fill\); \}/);
    expect(src('../components/Home.svelte')).toMatch(/\.card\.sel \{ --frame: var\(--accent-fill\); \}/);
  });
  it('dialogs and popovers have no UA border in the text colour', () => {
    expect(src('../app.css')).toMatch(/dialog, \[popover\] \{ border: 0; outline: 0; \}/);
  });
  it('rail tiles: no hover / picked ring, only the open (or Home) instance is framed', () => {
    const r = src('../components/Rail.svelte');
    expect(r).not.toMatch(/\.prof:hover \.ring/); expect(r).not.toMatch(/picked/); expect(r).toMatch(/li\.sel \.ring \{ background: var\(--accent-fill\); \}/);
  });
  it('rail has a plain + button above the account head', () => {
    const r = src('../components/Rail.svelte');
    expect(r).toMatch(/PlusIcon/); expect(r.indexOf('class="nav add"')).toBeLessThan(r.indexOf('class="avatar"'));
    expect(r).toMatch(/nav\.newProfile = true/);
  });
});

describe('first run without instances', () => {
  it('onboarding opens the creation dialog when there is no instance; Home and Library have the create call to action', () => {
    expect(src('../components/Onboarding.svelte')).toMatch(/app\.profiles\.length === 0\) nav\.newProfile = true/);
    expect(src('../components/Home.svelte')).toMatch(/home\.createInstance/);
    expect(src('../components/Library.svelte')).toMatch(/home\.createInstance/);
  });
});
