import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { BusyGate, createLoadState, DEFAULT_LOAD_TIMEOUT_MS, LOAD_TIMEOUT_MS, SKELETON_DELAY_MS, SKELETON_MIN_VISIBLE_MS } from './loadState.svelte';

const deferred = <T>() => {
  let resolve!: (v: T) => void; let reject!: (e: unknown) => void;
  const promise = new Promise<T>((res, rej) => { resolve = res; reject = rej; });
  return { promise, resolve, reject };
};
const tick = () => vi.advanceTimersByTimeAsync(0);

describe('LoadState', () => {
  beforeEach(() => { vi.useFakeTimers(); });
  afterEach(() => { vi.useRealTimers(); });

  it('uses 150 ms / 250 ms / 12 s by default', () => {
    expect([SKELETON_DELAY_MS, SKELETON_MIN_VISIBLE_MS, DEFAULT_LOAD_TIMEOUT_MS, LOAD_TIMEOUT_MS]).toEqual([150, 250, 12000, 12000]);
  });

  it('a fast load never shows the skeleton', async () => {
    const s = createLoadState<number[]>();
    const d = deferred<number[]>();
    s.run(() => d.promise);
    expect(s.phase).toBe('loading');
    await vi.advanceTimersByTimeAsync(100);
    expect(s.showSkeleton).toBe(false);
    d.resolve([1]);
    await tick();
    expect(s.phase).toBe('ready');
    expect(s.data).toEqual([1]);
    expect(s.showSkeleton).toBe(false);
    await vi.advanceTimersByTimeAsync(500);
    expect(s.showSkeleton).toBe(false); // the delay timer was cancelled
  });

  it('shows the skeleton after the delay and keeps it for the minimum visible time', async () => {
    const s = createLoadState<number[]>();
    const d = deferred<number[]>();
    s.run(() => d.promise);
    await vi.advanceTimersByTimeAsync(150);
    expect(s.showSkeleton).toBe(true);
    await vi.advanceTimersByTimeAsync(50); // 200 ms in, the answer arrives 50 ms after the skeleton appeared
    d.resolve([1, 2]);
    await tick();
    expect(s.phase).toBe('loading'); // held
    expect(s.showSkeleton).toBe(true);
    await vi.advanceTimersByTimeAsync(199);
    expect(s.showSkeleton).toBe(true);
    await vi.advanceTimersByTimeAsync(2);
    expect(s.phase).toBe('ready');
    expect(s.showSkeleton).toBe(false);
  });

  it('a slow answer that arrives after the minimum time is applied at once', async () => {
    const s = createLoadState<string>({ isEmpty: () => false });
    const d = deferred<string>();
    s.run(() => d.promise);
    await vi.advanceTimersByTimeAsync(1000);
    d.resolve('x');
    await tick();
    expect(s.phase).toBe('ready');
    expect(s.showSkeleton).toBe(false);
  });

  it('times out into an error with a retry that runs the loader again', async () => {
    const s = createLoadState<string>({ timeout: 1000 });
    let calls = 0;
    const loader = () => (++calls === 1 ? new Promise<string>(() => {}) : Promise.resolve('ok'));
    s.run(loader);
    await vi.advanceTimersByTimeAsync(999);
    expect(s.phase).toBe('loading');
    await vi.advanceTimersByTimeAsync(2);
    expect(s.phase).toBe('error');
    expect(s.error?.code).toBe('net.timeout');
    expect(s.showSkeleton).toBe(false);
    s.retry();
    expect(s.phase).toBe('loading');
    await vi.advanceTimersByTimeAsync(10);
    expect(s.phase).toBe('ready');
    expect(s.data).toBe('ok');
    expect(calls).toBe(2);
  });

  it('a timed-out load ignores its late answer and aborts its signal', async () => {
    const s = createLoadState<string>({ timeout: 500 });
    const d = deferred<string>();
    let signal!: AbortSignal;
    s.run((sig) => { signal = sig; return d.promise; });
    await vi.advanceTimersByTimeAsync(600);
    expect(signal.aborted).toBe(true);
    d.resolve('late');
    await tick();
    expect(s.phase).toBe('error');
    expect(s.data).toBeNull();
  });

  it('a failing loader becomes an error with its code (after the minimum time when the skeleton was showing)', async () => {
    const s = createLoadState<string>();
    const d = deferred<string>();
    s.run(() => d.promise);
    await vi.advanceTimersByTimeAsync(160);
    d.reject({ code: 'net.request', message: 'x', needsRelogin: false });
    await tick();
    expect(s.phase).toBe('loading');
    await vi.advanceTimersByTimeAsync(300);
    expect(s.phase).toBe('error');
    expect(s.error?.code).toBe('net.request');
  });

  it('cancels stale loads: the older answer never overwrites the newer one, and its signal is aborted', async () => {
    const s = createLoadState<string>({ isEmpty: () => false });
    const a = deferred<string>(); const b = deferred<string>();
    let sigA!: AbortSignal;
    s.run((sig) => { sigA = sig; return a.promise; });
    s.run(() => b.promise);
    expect(sigA.aborted).toBe(true);
    b.resolve('new');
    await tick();
    a.resolve('old');
    await tick();
    expect(s.data).toBe('new');
    expect(s.phase).toBe('ready');
  });

  it('cancel() drops the in-flight answer and returns to idle', async () => {
    const s = createLoadState<string>();
    const d = deferred<string>();
    s.run(() => d.promise);
    s.cancel();
    d.resolve('x');
    await vi.advanceTimersByTimeAsync(1000);
    expect(s.phase).toBe('idle');
    expect(s.data).toBeNull();
    expect(s.showSkeleton).toBe(false);
  });

  it('distinguishes empty, ready and error', async () => {
    const e = createLoadState<number[]>();
    e.run(async () => []);
    await tick();
    expect(e.phase).toBe('empty');
    const r = createLoadState<number[]>();
    r.run(async () => [1]);
    await tick();
    expect(r.phase).toBe('ready');
    const x = createLoadState<number[]>();
    x.run(async () => { throw { code: 'unknown', message: 'no', needsRelogin: false }; });
    await tick();
    expect(x.phase).toBe('error');
    expect(x.data).toBeNull();
  });

  it('a silent refresh keeps the data, never shows a skeleton and reports refreshing / refreshError', async () => {
    const s = createLoadState<string[]>();
    s.run(async () => ['a']);
    await tick();
    const d = deferred<string[]>();
    s.run(() => d.promise, { silent: true });
    expect(s.refreshing).toBe(true);
    expect(s.phase).toBe('ready');
    await vi.advanceTimersByTimeAsync(500);
    expect(s.showSkeleton).toBe(false);
    d.resolve(['a', 'b']);
    await tick();
    expect(s.refreshing).toBe(false);
    expect(s.data).toEqual(['a', 'b']);
    s.run(async () => { throw { code: 'net.request', message: '', needsRelogin: false }; }, { silent: true });
    await tick();
    expect(s.data).toEqual(['a', 'b']);
    expect(s.refreshError?.code).toBe('net.request');
    expect(s.phase).toBe('ready');
  });
});

describe('BusyGate', () => {
  beforeEach(() => { vi.useFakeTimers(); });
  afterEach(() => { vi.useRealTimers(); });
  it('delays the show, holds it for the minimum time, and never flashes for a short busy period', () => {
    const g = new BusyGate();
    g.update(true);
    vi.advanceTimersByTime(100);
    g.update(false);
    vi.advanceTimersByTime(1000);
    expect(g.show).toBe(false);
    g.update(true);
    vi.advanceTimersByTime(150);
    expect(g.show).toBe(true);
    vi.advanceTimersByTime(50);
    g.update(false);
    expect(g.show).toBe(true);
    vi.advanceTimersByTime(201);
    expect(g.show).toBe(false);
  });
});
