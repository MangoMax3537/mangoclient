// Background pool + Home cycling (UI rules for "auto" backgrounds).
//   Pool = generic wallpapers + one panorama set per installed version (panorama_pool) + the brand surface.
//   Instance hero, kind 'auto': (a) wallpaper whose manifest versions rule matches the Minecraft version, (b) the instance's own
//   panorama, (c) deterministic pool member by hash of the profile id.
//   Home: when the instance's background is 'auto', crossfades through the shuffled pool every 30 s.
import { hashId } from './fallbackImage';
import { versionMatches, type Wallpaper } from './wallpapers';
import type { PanoramaSet } from './types';

export type PoolItem =
  | { kind: 'wallpaper'; id: string; url: string; scrim: number }
  | { kind: 'panorama'; id: string; set: PanoramaSet }
  | { kind: 'brand'; id: 'brand' };

export const poolKey = (i: PoolItem): string => (i.kind === 'wallpaper' ? `w:${i.id}` : i.kind === 'panorama' ? `p:${i.id}` : 'brand');

/** Generic wallpapers, the panorama sets and the brand surface, in a stable order. */
export function buildPool(wallpapers: readonly Wallpaper[], sets: readonly PanoramaSet[], own: Wallpaper | null = null): PoolItem[] {
  const pool: PoolItem[] = [];
  const seen = new Set<string>();
  // the instance's own version wallpaper always takes part, so the pool has at least 2 entries (it + the brand surface) even without generic art
  if (own) { seen.add(own.id); pool.push({ kind: 'wallpaper', id: own.id, url: own.url, scrim: own.scrim }); }
  for (const w of wallpapers) if (w.generic && !seen.has(w.id)) { seen.add(w.id); pool.push({ kind: 'wallpaper', id: w.id, url: w.url, scrim: w.scrim }); }
  for (const s of sets) if (s.faces.length === 6 && !pool.some((p) => p.kind === 'panorama' && p.id === s.mcVersion)) pool.push({ kind: 'panorama', id: s.mcVersion, set: s });
  pool.push({ kind: 'brand', id: 'brand' });
  return pool;
}

/** (a) rule-matching wallpaper (several: hash pick), else null. */
export function versionWallpaper(list: readonly Wallpaper[], mcVersion: string, profileId: string): Wallpaper | null {
  const v = list.filter((w) => !w.generic && w.versions.some((p) => versionMatches(p, mcVersion)));
  return v.length ? v[hashId(profileId) % v.length] : null;
}

export interface AutoCtx { mcVersion: string; profileId: string; wallpapers: readonly Wallpaper[]; ownPanorama: PanoramaSet | null | undefined; pool: readonly PoolItem[] }
export type AutoRuleHit = 'version' | 'panorama' | 'pool';
/** The instance's own hero for kind 'auto': (a) version wallpaper, (b) its own panorama, (c) hash pick from the pool. */
export function resolveAuto(c: AutoCtx): { item: PoolItem; rule: AutoRuleHit } {
  const w = versionWallpaper(c.wallpapers, c.mcVersion, c.profileId);
  if (w) return { item: { kind: 'wallpaper', id: w.id, url: w.url, scrim: w.scrim }, rule: 'version' };
  if (c.ownPanorama && c.ownPanorama.faces.length === 6) return { item: { kind: 'panorama', id: c.ownPanorama.mcVersion, set: c.ownPanorama }, rule: 'panorama' };
  const pool = c.pool.length ? c.pool : buildPool([], []);
  return { item: pool[hashId(c.profileId) % pool.length], rule: 'pool' };
}

/* ---- the cycle ---- */
/** Fixed interval between two background changes. */
export const CYCLE_MS = 30_000;
export const FADE_MS = 1800;

/** Delay per cycle: fixed (no jitter). */
export const nextDelay = (): number => CYCLE_MS;

/** Fisher-Yates over a copy. */
export function shuffle<T>(a: readonly T[], rand: () => number = Math.random): T[] {
  const o = [...a];
  for (let i = o.length - 1; i > 0; i--) { const j = Math.floor(rand() * (i + 1)); [o[i], o[j]] = [o[j], o[i]]; }
  return o;
}

/** Shuffled order that never repeats an item back to back, also across the wrap (the new first is never the previous last). */
export class Order {
  #items: PoolItem[] = [];
  #queue: PoolItem[] = [];
  #last: string | null = null;
  constructor(private rand: () => number = Math.random) {}
  setPool(pool: readonly PoolItem[]): void {
    const keys = pool.map(poolKey).join('|');
    if (keys === this.#items.map(poolKey).join('|')) return;
    this.#items = [...pool];
    this.#queue = this.#queue.filter((q) => this.#items.some((i) => poolKey(i) === poolKey(q)));
  }
  get size(): number { return this.#items.length; }
  /** Marks what is on screen now so it is not picked next. */
  seed(key: string | null): void { this.#last = key; this.#queue = this.#queue.filter((q) => poolKey(q) !== key); }
  next(): PoolItem | null {
    if (this.#items.length < 2) return null;
    if (!this.#queue.length) {
      let q = shuffle(this.#items, this.rand);
      if (q.length > 1 && poolKey(q[0]) === this.#last) { const j = 1 + Math.floor(this.rand() * (q.length - 1)); [q[0], q[j]] = [q[j], q[0]]; }
      this.#queue = q;
    }
    const n = this.#queue.shift() as PoolItem;
    this.#last = poolKey(n);
    return n;
  }
}

export interface SchedulerDeps {
  setTimeout: (fn: () => void, ms: number) => unknown;
  clearTimeout: (h: unknown) => void;
  rand?: () => number;
  /** Overrides the delay (dev flag `?mockCycleMs=`). */
  fixedMs?: number | null;
}

/**
 * The timer behind Home cycling. `onNext` is called when it is time to move on; the caller loads + decodes the next image and then calls
 * `done()` which arms the next cycle. Paused (timer cleared, a full fresh interval on resume) while hidden / blurred.
 */
export class CycleScheduler {
  #h: unknown = null;
  #running = false;
  #paused = true;
  #waiting = false;
  constructor(private deps: SchedulerDeps, private onNext: () => void) {}
  get armed(): boolean { return this.#h !== null; }
  get running(): boolean { return this.#running; }
  start(): void { this.#running = true; this.#waiting = false; this.#arm(); }
  stop(): void { this.#running = false; this.#clear(); }
  /** `paused` = hidden document, blurred window or Home not shown. */
  setPaused(p: boolean): void { this.#paused = p; if (p) this.#clear(); else this.#arm(); }
  /** The swap finished (or failed): arm the next cycle. */
  done(): void { this.#waiting = false; this.#arm(); }
  #clear(): void { if (this.#h !== null) { this.deps.clearTimeout(this.#h); this.#h = null; } }
  #arm(): void {
    if (!this.#running || this.#paused || this.#waiting || this.#h !== null) return;
    const ms = this.deps.fixedMs && this.deps.fixedMs > 0 ? this.deps.fixedMs : nextDelay();
    this.#h = this.deps.setTimeout(() => { this.#h = null; this.#waiting = true; this.onNext(); }, ms);
  }
}
