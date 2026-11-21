// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { flushSync, mount, unmount } from 'svelte';
import { readFileSync } from 'node:fs';

const made = vi.hoisted(() => ({ n: 0 }));
vi.mock('../lib/bgCycle', async (orig) => {
  const m = await orig<typeof import('../lib/bgCycle')>();
  class Counting extends m.CycleScheduler { constructor(...a: ConstructorParameters<typeof m.CycleScheduler>) { super(...a); made.n++; } }
  return { ...m, CycleScheduler: Counting };
});
import Harness from './testing/BgShellHarness.svelte';
import { createMockApi } from '../lib/mockApi';
import { app, init, nav, resetStores, ui } from '../lib/stores.svelte';

beforeEach(() => {
  Object.defineProperty(HTMLImageElement.prototype, 'complete', { configurable: true, get: () => true });
  Object.defineProperty(HTMLImageElement.prototype, 'naturalWidth', { configurable: true, get: () => 64 });
  (HTMLImageElement.prototype as unknown as { decode: () => Promise<void> }).decode = () => Promise.resolve();
  vi.stubGlobal('requestAnimationFrame', (f: FrameRequestCallback) => setTimeout(() => f(0), 16));
  vi.stubGlobal('matchMedia', () => ({ matches: false, addEventListener() {}, removeEventListener() {} }));
  (HTMLCanvasElement.prototype as unknown as { getContext: () => unknown }).getContext = () => new Proxy({}, { get: () => () => undefined, set: () => true });
  (HTMLCanvasElement.prototype as unknown as { toDataURL: () => string }).toDataURL = () => 'data:image/png;base64,';
  vi.useFakeTimers();
  history.replaceState(null, '', '/');
  resetStores();
  ui.cycleBackgrounds = true;
  made.n = 0;
});
afterEach(() => { vi.useRealTimers(); vi.unstubAllGlobals(); });

const srcs = (root: ParentNode) => root.querySelector('[data-bg-engine]')?.getAttribute('data-slide') ?? '';

describe('global background across navigation', () => {
  it('keeps one engine and one timer, and changes the slide every 30-60 s on every view and instance tab switch', async () => {
    await init(createMockApi());
    const target = document.createElement('div'); document.body.append(target);
    const app_ = mount(Harness, { target });
    flushSync();
    await vi.advanceTimersByTimeAsync(100);
    const engine = target.querySelector('[data-bg-engine]');
    expect(engine).toBeTruthy();
    expect(made.n).toBe(1);

    const pid = app.profiles[0].id;
    const route: Array<() => void> = [
      () => { nav.view = 'library'; },
      () => { nav.view = 'profile'; nav.profileId = pid; },
      () => { nav.profileTab = 'content' as typeof nav.profileTab; },
      () => { nav.profileTab = 'settings' as typeof nav.profileTab; },
      () => { nav.view = 'discover'; },
      () => { nav.view = 'skins'; },
      () => { nav.view = 'settings'; },
      () => { nav.view = 'home'; },
    ];
    let seen = srcs(target);
    const changes: number[] = [];
    for (let i = 0; i < route.length; i++) {
      route[i](); flushSync();
      await vi.advanceTimersByTimeAsync(100);
      // the navigation itself never changes the slide, the engine node is the same, the scheduler is not recreated
      expect(srcs(target)).toBe(seen);
      expect(target.querySelector('[data-bg-engine]')).toBe(engine);
      expect(made.n).toBe(1);
      // the timer keeps running: within one 30-60 s interval (+ fade) the slide changes
      await vi.advanceTimersByTimeAsync(60_000 + 2_500);
      const now = srcs(target);
      changes.push(now !== seen ? 1 : 0);
      seen = now;
    }
    // every interval changed the slide (a shuffled pool never repeats the current slide directly)
    expect(changes).toEqual(route.map(() => 1));
    expect(made.n).toBe(1);
    unmount(app_);
  });

  it('App.svelte mounts the engine for every view without a cycle opt-out', () => {
    const src = readFileSync(`${process.cwd()}/src/App.svelte`, 'utf8');
    expect(src).toMatch(/<AppBackground[^>]*\/>/);
    expect(src.match(/<AppBackground[^>]*\/>/)?.[0]).not.toMatch(/cycle=/);
  });
});
