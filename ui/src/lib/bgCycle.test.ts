import { describe, expect, it, vi } from 'vitest';
import { CYCLE_MS, CycleScheduler, Order, buildPool, nextDelay, poolKey, resolveAuto, shuffle, versionWallpaper } from './bgCycle';
import { buildWallpapers } from './wallpapers';
import type { PanoramaSet } from './types';

const walls = buildWallpapers(
  { wallpapers: [{ file: 'pale', label: 'Pale', versions: ['1.21.4'], scrim: 0.4 }, { file: 'badlands', label: 'B', generic: true }, { file: 'taiga', label: 'T', generic: true }] },
  { '/a/pale.webp': 'pale.url', '/a/badlands.jpg': 'bad.url', '/a/taiga.jpg': 'taiga.url' },
);
const set = (v: string): PanoramaSet => ({ mcVersion: v, source: 'own', faces: ['a', 'b', 'c', 'd', 'e', 'f'] });

describe('pool', () => {
  it('generic wallpapers + panorama sets + the brand surface; version-specific wallpapers are not pool members', () => {
    const pool = buildPool(walls, [set('1.21.4'), set('1.20.6'), { ...set('1.8.9'), faces: ['x'] }]);
    expect(pool.map(poolKey)).toEqual(['w:badlands', 'w:taiga', 'p:1.21.4', 'p:1.20.6', 'brand']);
  });
  it('an empty world is just the brand surface', () => { expect(buildPool([], []).map(poolKey)).toEqual(['brand']); });
});

describe('auto resolution on an instance hero', () => {
  const pool = buildPool(walls, [set('1.20.6')]);
  it('(a) version rule first', () => {
    const r = resolveAuto({ mcVersion: '1.21.4', profileId: 'p', wallpapers: walls, ownPanorama: set('1.21.4'), pool });
    expect(r.rule).toBe('version'); expect(r.item).toMatchObject({ kind: 'wallpaper', id: 'pale' });
  });
  it('(b) the own panorama when no version rule matches', () => {
    const r = resolveAuto({ mcVersion: '1.20.6', profileId: 'p', wallpapers: walls, ownPanorama: set('1.20.6'), pool });
    expect(r.rule).toBe('panorama'); expect(r.item).toMatchObject({ kind: 'panorama', id: '1.20.6' });
  });
  it('(c) a deterministic pick from the pool while the own panorama is not there (null) - same id, same pick', () => {
    const a = resolveAuto({ mcVersion: '1.7.10', profileId: 'abc', wallpapers: walls, ownPanorama: null, pool });
    const b = resolveAuto({ mcVersion: '1.7.10', profileId: 'abc', wallpapers: walls, ownPanorama: undefined, pool });
    expect(a.rule).toBe('pool'); expect(poolKey(a.item)).toBe(poolKey(b.item));
    const picks = new Set(['a', 'b', 'c', 'd', 'e', 'f', 'g', 'h'].map((id) => poolKey(resolveAuto({ mcVersion: '1.7.10', profileId: id, wallpapers: walls, ownPanorama: null, pool }).item)));
    expect(picks.size).toBeGreaterThan(1);
  });
  it('an empty pool still resolves (brand surface)', () => {
    expect(resolveAuto({ mcVersion: '1.7.10', profileId: 'p', wallpapers: [], ownPanorama: null, pool: [] }).item.kind).toBe('brand');
    expect(versionWallpaper(walls, '1.7.10', 'p')).toBeNull();
  });
});

describe('Order (shuffle without repeats)', () => {
  const pool = buildPool(walls, [set('1.21.4'), set('1.20.6')]);
  it('visits every member before repeating and never shows the same one twice in a row, also across the wrap', () => {
    const o = new Order(); o.setPool(pool); o.seed('w:badlands');
    let prev = 'w:badlands';
    const seen: string[] = [];
    for (let i = 0; i < 60; i++) { const n = poolKey(o.next()!); expect(n).not.toBe(prev); prev = n; seen.push(n); }
    expect(new Set(seen).size).toBe(pool.length);
    // first round after the seed covers all but the seed
    expect(new Set(seen.slice(0, pool.length - 1)).size).toBe(pool.length - 1);
  });
  it('needs two members to cycle', () => {
    const o = new Order(); o.setPool(buildPool([], []));
    expect(o.next()).toBeNull();
  });
  it('shuffle keeps all items', () => { expect(shuffle([1, 2, 3, 4, 5]).sort()).toEqual([1, 2, 3, 4, 5]); });
});

describe('pool with only a version wallpaper', () => {
  const own = { id: 'v', url: 'u', scrim: 0.4, generic: false, versions: ['1.21'] } as unknown as Parameters<typeof buildPool>[0][number];
  it('has at least 2 entries: the version wallpaper and the brand surface', () => {
    const pool = buildPool([], [], own);
    expect(pool.length).toBeGreaterThanOrEqual(2);
    expect(pool.map(poolKey)).toEqual(['w:v', 'brand']);
  });
  it('does not list the same wallpaper twice when it is also generic', () => {
    const g = { ...own, generic: true } as typeof own;
    expect(buildPool([g], [], g).map(poolKey)).toEqual(['w:v', 'brand']);
  });
});

describe('delay', () => {
  it('is a fixed 30 s', () => {
    expect(CYCLE_MS).toBe(30000);
    for (let i = 0; i < 20; i++) expect(nextDelay()).toBe(30000);
  });
});

describe('CycleScheduler (fake timers)', () => {
  const make = (o: { rand?: () => number; fixedMs?: number } = {}) => {
    const next = vi.fn();
    const s = new CycleScheduler({ setTimeout: (f, ms) => setTimeout(f, ms), clearTimeout: (h) => clearTimeout(h as ReturnType<typeof setTimeout>), ...o }, next);
    return { s, next };
  };
  it('fires after the interval, then waits for done() before arming the next', () => {
    vi.useFakeTimers();
    const { s, next } = make({ rand: () => 0 });
    s.start(); s.setPaused(false);
    vi.advanceTimersByTime(CYCLE_MS - 1); expect(next).not.toHaveBeenCalled();
    vi.advanceTimersByTime(1); expect(next).toHaveBeenCalledTimes(1);
    vi.advanceTimersByTime(10 * CYCLE_MS); expect(next).toHaveBeenCalledTimes(1); // still decoding / fading
    s.done(); vi.advanceTimersByTime(CYCLE_MS); expect(next).toHaveBeenCalledTimes(2);
    vi.useRealTimers();
  });
  it('uses the same 30 s delay every cycle', () => {
    vi.useFakeTimers();
    const { s, next } = make();
    s.start(); s.setPaused(false);
    vi.advanceTimersByTime(30000); expect(next).toHaveBeenCalledTimes(1);
    s.done(); vi.advanceTimersByTime(29999); expect(next).toHaveBeenCalledTimes(1);
    vi.advanceTimersByTime(1); expect(next).toHaveBeenCalledTimes(2);
    vi.useRealTimers();
  });
  it('pauses (hidden / blurred / Home not shown) and resumes with a fresh delay', () => {
    vi.useFakeTimers();
    const { s, next } = make({ rand: () => 0 });
    s.start(); s.setPaused(false);
    vi.advanceTimersByTime(20000);
    s.setPaused(true); expect(s.armed).toBe(false);
    vi.advanceTimersByTime(5 * CYCLE_MS); expect(next).not.toHaveBeenCalled();
    s.setPaused(false); expect(s.armed).toBe(true);
    vi.advanceTimersByTime(CYCLE_MS); expect(next).toHaveBeenCalledTimes(1);
    vi.useRealTimers();
  });
  it('does not run before start, and stop cancels', () => {
    vi.useFakeTimers();
    const { s, next } = make({ rand: () => 0 });
    s.setPaused(false); vi.advanceTimersByTime(CYCLE_MS * 2); expect(next).not.toHaveBeenCalled();
    s.start(); s.stop(); vi.advanceTimersByTime(CYCLE_MS * 2); expect(next).not.toHaveBeenCalled();
    vi.useRealTimers();
  });
  it('fixedMs (dev flag) overrides the interval', () => {
    vi.useFakeTimers();
    const { s, next } = make({ fixedMs: 3000 });
    s.start(); s.setPaused(false); vi.advanceTimersByTime(3000); expect(next).toHaveBeenCalledTimes(1);
    vi.useRealTimers();
  });
});

import { resolveBackground, choiceFromBackground } from './instanceIcon';
import { MANGO_CLIENT_ID } from './wallpapers';
describe('built-in Mango Client wallpaper', () => {
  const ctx = { mcVersion: '1.21.4', profileId: 'p', panorama: null, wallpapers: [] as never[] };
  it('fits the backend wallpaper-id rule ([a-z0-9-], 1..64)', () => { expect(MANGO_CLIENT_ID).toMatch(/^[a-z0-9-]{1,64}$/); });
  it('wallpaper:mango-client resolves to the brand surface even without any image file', () => {
    expect(resolveBackground({ kind: 'wallpaper', id: MANGO_CLIENT_ID }, ctx)).toEqual({ kind: 'watermark' });
  });
  it('the picker keeps it as a wallpaper choice (not "auto")', () => {
    expect(choiceFromBackground({ kind: 'wallpaper', id: MANGO_CLIENT_ID }, [])).toMatchObject({ kind: 'wallpaper', id: 'mango-client' });
  });
  it('it is one member of the auto / Home pool', () => { expect(buildPool([], []).map(poolKey)).toContain('brand'); });
});

describe('CycleScheduler restart', () => {
  it('re-arms after stop() mid-swap (a stale waiting flag must not block it) and when started late', () => {
    vi.useFakeTimers();
    const next = vi.fn();
    const s = new CycleScheduler({ setTimeout: (f, ms) => setTimeout(f, ms), clearTimeout: (h) => clearTimeout(h as ReturnType<typeof setTimeout>) }, next);
    s.setPaused(false);
    vi.advanceTimersByTime(CYCLE_MS * 3); expect(next).not.toHaveBeenCalled(); // not started yet (no profile)
    s.start(); vi.advanceTimersByTime(CYCLE_MS); expect(next).toHaveBeenCalledTimes(1); // now waiting for done()
    s.stop(); s.start(); vi.advanceTimersByTime(CYCLE_MS); expect(next).toHaveBeenCalledTimes(2);
    vi.useRealTimers();
  });
});
