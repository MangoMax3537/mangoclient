import { describe, expect, it, vi } from 'vitest';
import { CYCLE_MS, CycleScheduler, Order, buildPool, nextDelay, poolKey, resolveAuto, shuffle, versionWallpaper } from './bgCycle';
import { buildWallpapers } from './wallpapers';

const walls = buildWallpapers(
  { wallpapers: [{ file: 'pale', label: 'Pale', versions: ['1.21.4'], scrim: 0.4 }, { file: 'badlands', label: 'B', generic: true }, { file: 'taiga', label: 'T', generic: true }] },
  { '/a/pale.webp': 'pale.url', '/a/badlands.jpg': 'bad.url', '/a/taiga.jpg': 'taiga.url' },
);

describe('pool', () => {
  it('only bundled generic wallpapers (no brand surface, no runtime panoramas); version-specific wallpapers are not pool members', () => {
    expect(buildPool(walls).map(poolKey)).toEqual(['w:badlands', 'w:taiga']);
  });
  it('without any generic wallpaper every bundled one is in the pool, so auto is never left without an image', () => {
    const only = walls.filter((w) => !w.generic);
    expect(buildPool(only).map(poolKey)).toEqual(['w:pale']);
  });
  it('an empty world has an empty pool (the logo is never a background slide)', () => { expect(buildPool([])).toEqual([]); });
});

describe('auto resolution on an instance hero', () => {
  const pool = buildPool(walls);
  it('(a) version rule first', () => {
    const r = resolveAuto({ mcVersion: '1.21.4', profileId: 'p', wallpapers: walls, pool });
    expect(r.rule).toBe('version'); expect(r.item).toMatchObject({ kind: 'wallpaper', id: 'pale' });
  });
  it('(b) a deterministic pick of a bundled wallpaper, never a panorama - same id, same pick', () => {
    const a = resolveAuto({ mcVersion: '1.7.10', profileId: 'abc', wallpapers: walls, pool });
    const b = resolveAuto({ mcVersion: '1.7.10', profileId: 'abc', wallpapers: walls, pool });
    expect(a.rule).toBe('pool'); expect(poolKey(a.item)).toBe(poolKey(b.item));
    const picks = ['a', 'b', 'c', 'd', 'e', 'f', 'g', 'h'].map((id) => resolveAuto({ mcVersion: '1.7.10', profileId: id, wallpapers: walls, pool }).item);
    expect(new Set(picks.map(poolKey)).size).toBeGreaterThan(1);
    for (const p of picks) expect(p.kind).toBe('wallpaper');
  });
  it('an empty pool still resolves (the instance hero falls back to its own surface)', () => {
    expect(resolveAuto({ mcVersion: '1.7.10', profileId: 'p', wallpapers: [], pool: [] }).item.kind).toBe('brand');
    expect(versionWallpaper(walls, '1.7.10', 'p')).toBeNull();
  });
});

describe('Order (shuffle without repeats)', () => {
  const pool = buildPool(walls, walls[0]);
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
    const o = new Order(); o.setPool(buildPool([]));
    expect(o.next()).toBeNull();
  });
  it('shuffle keeps all items', () => { expect(shuffle([1, 2, 3, 4, 5]).sort()).toEqual([1, 2, 3, 4, 5]); });
});

describe('pool with only a version wallpaper', () => {
  const own = { id: 'v', url: 'u', scrim: 0.4, generic: false, versions: ['1.21'] } as unknown as Parameters<typeof buildPool>[0][number];
  it('contains the version wallpaper and never the brand', () => {
    const pool = buildPool([], own);
    expect(pool.map(poolKey)).toEqual(['w:v']);
  });
  it('does not list the same wallpaper twice when it is also generic', () => {
    const g = { ...own, generic: true } as typeof own;
    expect(buildPool([g], g).map(poolKey)).toEqual(['w:v']);
  });
});

describe('delay', () => {
  it('is 30 s plus a random 0-30 s', () => {
    expect(CYCLE_MS).toBe(30000);
    for (let i = 0; i < 50; i++) { const d = nextDelay(); expect(d).toBeGreaterThanOrEqual(30000); expect(d).toBeLessThan(60000); }
    expect(nextDelay(() => 0)).toBe(30000);
    expect(nextDelay(() => 1)).toBeLessThan(60000);
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
  it('waits 30-60 s per cycle (a new random delay each time)', () => {
    vi.useFakeTimers();
    const r = [0, 0.5];
    const { s, next } = make({ rand: () => r.shift() ?? 0 });
    s.start(); s.setPaused(false);
    vi.advanceTimersByTime(30000); expect(next).toHaveBeenCalledTimes(1);
    s.done(); vi.advanceTimersByTime(44999); expect(next).toHaveBeenCalledTimes(1);
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
  it('it is not a member of the auto / Home pool', () => { expect(buildPool([]).map(poolKey)).not.toContain('brand'); });
});

describe('CycleScheduler restart', () => {
  it('re-arms after stop() mid-swap (a stale waiting flag must not block it) and when started late', () => {
    vi.useFakeTimers();
    const next = vi.fn();
    const s = new CycleScheduler({ setTimeout: (f, ms) => setTimeout(f, ms), clearTimeout: (h) => clearTimeout(h as ReturnType<typeof setTimeout>), rand: () => 0 }, next);
    s.setPaused(false);
    vi.advanceTimersByTime(CYCLE_MS * 3); expect(next).not.toHaveBeenCalled(); // not started yet (no profile)
    s.start(); vi.advanceTimersByTime(CYCLE_MS); expect(next).toHaveBeenCalledTimes(1); // now waiting for done()
    s.stop(); s.start(); vi.advanceTimersByTime(CYCLE_MS); expect(next).toHaveBeenCalledTimes(2);
    vi.useRealTimers();
  });
});
