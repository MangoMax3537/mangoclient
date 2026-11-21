import { describe, expect, it } from 'vitest';
import { artUrl, floorWallpaper } from './bgFloor';
import type { Wallpaper } from './wallpapers';

const w = (id: string, generic: boolean): Wallpaper => ({ id, file: `${id}.jpg`, label: id, url: `/${id}.jpg`, path: '', versions: [], generic, scrim: 0.6 });
const list = [w('a', false), w('b', true), w('c', true)];

describe('bundled wallpaper floor', () => {
  it('uses the remembered wallpaper, even a version one', () => {
    expect(floorWallpaper(list, 'a')?.id).toBe('a');
  });
  it('falls back to a deterministic generic one for an unknown or missing memory', () => {
    const x = floorWallpaper(list, null);
    expect(x?.generic).toBe(true);
    expect(floorWallpaper(list, 'gone')?.id).toBe(x?.id);
    expect(floorWallpaper(list, null)?.id).toBe(x?.id);
  });
  it('takes any bundled wallpaper when none is generic and is null only without any', () => {
    expect(floorWallpaper([w('a', false)], null)?.id).toBe('a');
    expect(floorWallpaper([], null)).toBeNull();
  });
  it('identifies a slide by its first url', () => {
    expect(artUrl({ kind: 'image', url: '/x.jpg' })).toBe('/x.jpg');
    expect(artUrl({ kind: 'panorama', set: { faces: ['/f0', '/f1'] } })).toBe('/f0');
    expect(artUrl({ kind: 'plain' })).toBeNull();
  });
});

describe('bundled wallpapers ship with the app', async () => {
  const { existsSync, readFileSync, readdirSync } = await import('node:fs');
  const { fileURLToPath } = await import('node:url');
  const { wallpapers } = await import('./wallpapers');
  const { floorCandidates } = await import('./bgFloor');
  const bgDir = fileURLToPath(new URL('../../../assets/backgrounds/', import.meta.url));
  const dist = fileURLToPath(new URL('../../dist/', import.meta.url));

  it('every manifest entry has its image file and the bundle knows it', () => {
    const manifest = JSON.parse(readFileSync(`${bgDir}backgrounds.json`, 'utf8')) as { wallpapers: { file: string }[] };
    expect(manifest.wallpapers.length).toBeGreaterThan(0);
    for (const e of manifest.wallpapers) {
      const stem = e.file.replace(/\.(webp|jpe?g|png)$/i, '');
      expect(readdirSync(bgDir).some((f) => f.replace(/\.[^.]+$/, '') === stem && f !== 'backgrounds.json'), e.file).toBe(true);
      expect(wallpapers.some((w) => w.id === stem.toLowerCase()), `${e.file} bundled`).toBe(true);
    }
  });
  it('every URL the engine can draw is an app-origin path that exists in dist (when built) and passes the CSP img-src self rule', () => {
    for (const w of wallpapers) {
      expect(w.url, w.id).toMatch(/^(\/[^/]|\.\/)/); // same origin, never an http(s)/data/mangoimg URL
      if (existsSync(dist + 'assets')) {
        const stem = w.file.replace(/\.[^.]+$/, '');
        const ext = w.file.split('.').pop();
        expect(readdirSync(dist + 'assets').some((f) => f.startsWith(`${stem}-`) && f.endsWith(`.${ext}`)), `${w.file} in dist/assets`).toBe(true);
      }
    }
  });
  it('the floor has a next candidate for every failure and always at least one', () => {
    const c = floorCandidates(wallpapers, null);
    expect(c.length).toBe(wallpapers.length);
    expect(new Set(c.map((w) => w.id)).size).toBe(c.length);
  });
});
