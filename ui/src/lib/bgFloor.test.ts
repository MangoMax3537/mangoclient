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
