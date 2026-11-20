// One load-state machine for everything that fetches: idle -> loading -> ready | empty | error.
//  - `showSkeleton` turns true only after `delay` ms (no flash for fast loads) and stays true for at least `minVisible` ms once shown.
//  - after `timeout` ms the load is abandoned: phase `error` with code `net.timeout` (late results are ignored), `retry()` runs it again.
//  - every `run()` supersedes the previous one (and aborts its AbortSignal), so a slow stale answer can never overwrite a newer one.
import { untrack } from 'svelte';
import { toAppError } from './api';
import type { AppError } from './types';

export type LoadPhase = 'idle' | 'loading' | 'ready' | 'empty' | 'error';
export const SKELETON_DELAY_MS = 150;
export const SKELETON_MIN_VISIBLE_MS = 250;
export const DEFAULT_LOAD_TIMEOUT_MS = 12_000;
/** 12 s; dev builds can shorten it with `?loadTimeout=<ms>` to see the timeout -> error -> retry flow quickly. */
export const LOAD_TIMEOUT_MS: number = (() => {
  try {
    const v = Number(new URLSearchParams(globalThis.location?.search ?? '').get('loadTimeout'));
    return Number.isFinite(v) && v > 0 ? v : DEFAULT_LOAD_TIMEOUT_MS;
  } catch { return DEFAULT_LOAD_TIMEOUT_MS; }
})();

export interface LoadOptions<T> {
  delay?: number;
  minVisible?: number;
  timeout?: number;
  /** Decides `empty` vs `ready`. Default: empty arrays only. */
  isEmpty?: (value: T) => boolean;
}
export interface RunOptions {
  /** Refresh: keep showing the current data, never show a skeleton, swap the data when the answer arrives. */
  silent?: boolean;
}
export type Loader<T> = (signal: AbortSignal) => Promise<T>;

export class LoadState<T> {
  phase = $state<LoadPhase>('idle');
  data = $state.raw<T | null>(null);
  error = $state.raw<AppError | null>(null);
  /** True while the skeleton should be on screen (loading, past the delay or still inside the minimum visible time). */
  showSkeleton = $state(false);
  /** A silent refresh (or load-more) is in flight; the old data stays visible. */
  refreshing = $state(false);
  /** The last silent refresh failed or timed out (the old data is kept). */
  refreshError = $state.raw<AppError | null>(null);

  #o: Required<Omit<LoadOptions<T>, 'isEmpty'>> & { isEmpty: (v: T) => boolean };
  #seq = 0;
  #loader: Loader<T> | null = null;
  #ctl: AbortController | null = null;
  #timers = new Set<ReturnType<typeof setTimeout>>();

  constructor(o: LoadOptions<T> = {}) {
    this.#o = {
      delay: o.delay ?? SKELETON_DELAY_MS, minVisible: o.minVisible ?? SKELETON_MIN_VISIBLE_MS, timeout: o.timeout ?? LOAD_TIMEOUT_MS,
      isEmpty: o.isEmpty ?? ((v) => Array.isArray(v) && v.length === 0),
    };
  }

  get loading(): boolean { return this.phase === 'loading'; }

  #later(ms: number, fn: () => void): void {
    const id = setTimeout(() => { this.#timers.delete(id); fn(); }, ms);
    this.#timers.add(id);
  }
  #clear(): void {
    for (const id of this.#timers) clearTimeout(id);
    this.#timers.clear();
  }

  /** Starts (or restarts) a load. Safe to call from an `$effect`: it never reads its own state reactively. */
  run(loader: Loader<T>, opts: RunOptions = {}): void {
    untrack(() => {
      this.#loader = loader;
      this.#clear();
      this.#ctl?.abort();
      const ctl = (this.#ctl = new AbortController());
      const my = ++this.#seq;
      const silent = !!opts.silent && this.data !== null;
      let shownAt = 0;
      let settled = false;
      if (silent) { this.refreshing = true; this.refreshError = null; }
      else {
        this.refreshing = false; this.refreshError = null;
        this.phase = 'loading'; this.error = null; this.showSkeleton = false;
        this.#later(this.#o.delay, () => { if (my === this.#seq && !settled) { this.showSkeleton = true; shownAt = Date.now(); } });
      }
      const fail = (e: AppError) => {
        if (silent) { this.refreshing = false; this.refreshError = e; } else this.#finish('error', null, e);
      };
      this.#later(this.#o.timeout, () => {
        if (my !== this.#seq || settled) return;
        settled = true; ctl.abort();
        fail({ code: 'net.timeout', message: 'timeout', needsRelogin: false });
      });
      const apply = (fn: () => void) => {
        if (my !== this.#seq || settled) return;
        settled = true;
        this.#clear();
        const wait = this.showSkeleton ? shownAt + this.#o.minVisible - Date.now() : 0;
        if (wait > 0) this.#later(wait, () => { if (my === this.#seq) fn(); });
        else fn();
      };
      let p: Promise<T>;
      try { p = loader(ctl.signal); } catch (e) { p = Promise.reject(e); }
      p.then(
        (v) => apply(() => { this.refreshing = false; this.#finish(this.#o.isEmpty(v) ? 'empty' : 'ready', v, null); }),
        (e) => apply(() => fail(toAppError(e))),
      );
    });
  }

  #finish(phase: LoadPhase, data: T | null, error: AppError | null): void {
    this.showSkeleton = false;
    this.#clear();
    this.phase = phase; this.data = data; this.error = error;
  }

  /** Runs the last loader again with the full skeleton cycle. */
  retry(): void { if (this.#loader) this.run(this.#loader); }

  /** Replace the data without a load (after a local edit). */
  set(data: T): void { this.refreshing = false; this.#seq++; this.#clear(); this.#ctl?.abort(); this.#finish(this.#o.isEmpty(data) ? 'empty' : 'ready', data, null); }

  /** Stops everything: pending timers, the in-flight load (its answer is ignored). Call from the effect cleanup. */
  cancel(): void {
    this.#seq++; this.#clear(); this.#ctl?.abort(); this.#ctl = null;
    this.showSkeleton = false; this.refreshing = false;
    if (this.phase === 'loading') this.phase = 'idle';
  }
}

export const createLoadState = <T>(o?: LoadOptions<T>): LoadState<T> => new LoadState<T>(o);

/**
 * Bridge for loads that are not a single promise (a `$state` flag some store flips, a paged list): a boolean "busy" signal becomes the same
 * delayed / minimum-time `show` flag. Call `update(busy)` whenever the flag changes.
 */
export class BusyGate {
  show = $state(false);
  #t: ReturnType<typeof setTimeout> | null = null;
  #since = 0;
  constructor(private delay = SKELETON_DELAY_MS, private minVisible = SKELETON_MIN_VISIBLE_MS) {}
  update(busy: boolean): void {
    untrack(() => {
      if (this.#t) { clearTimeout(this.#t); this.#t = null; }
      if (busy) {
        if (this.show) return;
        this.#t = setTimeout(() => { this.#t = null; this.show = true; this.#since = Date.now(); }, this.delay);
      } else if (this.show) {
        const wait = this.#since + this.minVisible - Date.now();
        if (wait > 0) this.#t = setTimeout(() => { this.#t = null; this.show = false; }, wait);
        else this.show = false;
      }
    });
  }
  destroy(): void { if (this.#t) clearTimeout(this.#t); this.#t = null; }
}
