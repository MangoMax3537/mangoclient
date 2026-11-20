import { describe, expect, it } from 'vitest';
import { autoWallpaper, buildWallpapers, clampScrim, uniqueWallpapers, versionMatches, wallpapers } from './wallpapers';

const imgs = (...names: string[]) => Object.fromEntries(names.map((n) => [`/x/assets/backgrounds/${n}`, `/u/${n}`]));
const manifest = {
  wallpapers: [
    { file: 'cherry-grove', label: 'Cherry Grove', versions: ['1.20.*'] },
    { file: 'trial-chambers', label: 'Trial', versions: ['1.21', '1.21.1'] },
    { file: 'pale-garden', label: 'Pale', versions: ['1.21.4'] },
    { file: 'sky-islands', label: 'Sky', versions: ['1.21.6-1.21.8'] },
    { file: 'dappled-forest-gold', label: 'Gold', versions: ['26.3*'] },
    { file: 'dappled-forest-dusk', label: 'Dusk', versions: ['26.3*'] },
    { file: 'deep-dark', label: 'Deep Dark', versions: ['1.19.*'], scrim: 0.1 },
    { file: 'badlands', label: 'Badlands', generic: true },
    { file: 'taiga-aurora', label: 'Taiga', generic: true },
    { file: 'ghost', label: 'Missing file', versions: ['1.8.9'] },
    { file: 'both', label: 'Both', versions: ['1.7.10'], generic: true },
  ],
};
const all = buildWallpapers(manifest, imgs('cherry-grove.webp', 'trial-chambers.jpg', 'pale-garden.png', 'sky-islands.jpeg', 'dappled-forest-gold.webp',
  'dappled-forest-dusk.webp', 'deep-dark.webp', 'badlands.webp', 'taiga-aurora.webp', 'both.webp', 'extra-shot.webp', 'notes.txt'));
const id = (mc: string, pid = 'p') => autoWallpaper(all, mc, pid);

describe('version rules', () => {
  it('exact ids', () => {
    expect(versionMatches('1.21.4', '1.21.4')).toBe(true);
    expect(versionMatches('1.21.4', '1.21.40')).toBe(false);
    expect(versionMatches('1.21', '1.21')).toBe(true);
    expect(versionMatches('1.21', '1.21.1')).toBe(false);
  });
  it('prefix wildcards', () => {
    expect(versionMatches('1.20.*', '1.20.6')).toBe(true);
    expect(versionMatches('1.20.*', '1.20')).toBe(true);
    expect(versionMatches('1.20.*', '1.200')).toBe(false);
    expect(versionMatches('1.20.*', '1.2.1')).toBe(false);
    expect(versionMatches('26.3*', '26.3.2')).toBe(true);
    expect(versionMatches('26.3*', '26.4')).toBe(false);
  });
  it('numeric-aware inclusive ranges', () => {
    const r = '1.21.6-1.21.8';
    expect(['1.21.5', '1.21.6', '1.21.7', '1.21.8', '1.21.9', '1.21.10'].map((v) => versionMatches(r, v))).toEqual([false, true, true, true, false, false]);
    expect(versionMatches('1.9-1.10', '1.10')).toBe(true);
    expect(versionMatches(r, '24w40a')).toBe(false);
  });
});

describe('auto wallpaper', () => {
  it('picks the version rule first', () => {
    expect(id('1.21.4')?.wallpaper.id).toBe('pale-garden');
    expect(id('1.21.4')?.rule).toBe('version');
    expect(id('1.20.6')?.wallpaper.id).toBe('cherry-grove');
    expect(id('1.21.7')?.wallpaper.id).toBe('sky-islands');
    expect(id('1.19.2')?.wallpaper.scrim).toBe(0.3);
  });
  it('several matches: deterministic pick by profile id hash, both reachable', () => {
    const picks = new Set<string>();
    for (let i = 0; i < 40; i++) {
      const a = id('26.3.1', `p-${i}`)!;
      expect(id('26.3.1', `p-${i}`)!.wallpaper.id).toBe(a.wallpaper.id);
      expect(a.rule).toBe('version');
      picks.add(a.wallpaper.id);
    }
    expect([...picks].sort()).toEqual(['dappled-forest-dusk', 'dappled-forest-gold']);
  });
  it('generic pool only when no version rule matches, deterministic per id', () => {
    const picks = new Set<string>();
    for (let i = 0; i < 40; i++) {
      const a = id('1.7.2', `p-${i}`)!;
      expect(a.rule).toBe('generic');
      expect(a.wallpaper.id).toBe(id('1.7.2', `p-${i}`)!.wallpaper.id);
      picks.add(a.wallpaper.id);
    }
    expect([...picks].sort()).toEqual(['badlands', 'taiga-aurora']);
    expect(id('1.20.1')?.rule).toBe('version');
  });
  it('an entry is never both versioned and generic', () => {
    const both = all.find((w) => w.id === 'both')!;
    expect(both.generic).toBe(false);
    expect(both.versions).toEqual(['1.7.10']);
    for (let i = 0; i < 40; i++) expect(id('1.7.2', `q-${i}`)!.wallpaper.id).not.toBe('both');
    expect(id('1.7.10')?.wallpaper.id).toBe('both');
  });
  it('snapshots fall back to the generic pool', () => {
    expect(id('24w40a')?.rule).toBe('generic');
  });
});

describe('manifest loading', () => {
  it('resolves the file stem against webp, jpg, jpeg and png', () => {
    expect(all.find((w) => w.id === 'trial-chambers')?.url).toBe('/u/trial-chambers.jpg');
    expect(all.find((w) => w.id === 'sky-islands')?.url).toBe('/u/sky-islands.jpeg');
    expect(all.find((w) => w.id === 'pale-garden')?.url).toBe('/u/pale-garden.png');
    expect(all.find((w) => w.id === 'pale-garden')?.path).toBe('assets/backgrounds/pale-garden.png');
  });
  it('skips entries whose file does not exist and ignores non-images', () => {
    expect(all.find((w) => w.id === 'ghost')).toBeUndefined();
    expect(all.find((w) => w.id === 'notes')).toBeUndefined();
  });
  it('lists images without a manifest entry, but never picks them automatically', () => {
    const extra = all.find((w) => w.id === 'extra-shot')!;
    expect(extra.label).toBe('Extra Shot');
    expect(extra.generic).toBe(false);
    expect(extra.versions).toEqual([]);
    expect(uniqueWallpapers([...all, ...all])).toHaveLength(all.length);
  });
  it('clamps and defaults the scrim', () => {
    expect(clampScrim(0.1)).toBe(0.3);
    expect(clampScrim(5)).toBe(0.9);
    expect(clampScrim(undefined)).toBe(0.6);
    expect(all.find((w) => w.id === 'pale-garden')?.scrim).toBe(0.6);
  });
  it('an empty folder, no manifest or garbage gives an empty list and no auto wallpaper', () => {
    expect(buildWallpapers(manifest, {})).toEqual([]);
    expect(buildWallpapers(undefined, undefined)).toEqual([]);
    expect(buildWallpapers({ wallpapers: 'x' }, {})).toEqual([]);
    expect(buildWallpapers(null, imgs('a.webp'))).toHaveLength(1);
    expect(autoWallpaper([], '1.21.4', 'p')).toBeNull();
  });
  it('the real folder loads without crashing', () => {
    expect(Array.isArray(wallpapers)).toBe(true);
  });
});
