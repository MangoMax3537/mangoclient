// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { flushSync, mount, unmount } from 'svelte';
import Harness from './testing/BgShellHarness.svelte';
import { createMockApi } from '../lib/mockApi';
import { app, init, nav, resetStores, ui } from '../lib/stores.svelte';
import { LAST_BG_KEY, floorWallpaper } from '../lib/bgFloor';
import { wallpapers } from '../lib/wallpapers';

type Behavior = 'ok' | 'fail' | 'delay';
let behave: (src: string) => Behavior = () => 'ok';
const ready = new Set<string>();
const srcOf = (el: HTMLImageElement) => el.getAttribute('src') ?? '';
const floorUrl = () => floorWallpaper(wallpapers, null)!.url;

beforeEach(() => {
  behave = () => 'ok'; ready.clear();
  const state = (el: HTMLImageElement) => { const b = behave(srcOf(el)); return b === 'delay' && ready.has(srcOf(el)) ? 'ok' : b; };
  Object.defineProperty(HTMLImageElement.prototype, 'complete', { configurable: true, get(this: HTMLImageElement) { return state(this) !== 'delay'; } });
  Object.defineProperty(HTMLImageElement.prototype, 'naturalWidth', { configurable: true, get(this: HTMLImageElement) { return state(this) === 'ok' ? 64 : 0; } });
  (HTMLImageElement.prototype as unknown as { decode: () => Promise<void> }).decode = () => Promise.resolve();
  vi.stubGlobal('requestAnimationFrame', (f: FrameRequestCallback) => setTimeout(() => f(0), 16));
  vi.stubGlobal('matchMedia', () => ({ matches: false, addEventListener() {}, removeEventListener() {} }));
  (HTMLCanvasElement.prototype as unknown as { getContext: () => unknown }).getContext = () => new Proxy({}, { get: () => () => undefined, set: () => true });
  (HTMLCanvasElement.prototype as unknown as { toDataURL: () => string }).toDataURL = () => 'data:image/png;base64,';
  vi.useFakeTimers();
  history.replaceState(null, '', '/?mockCycleMs=1500');
  try { localStorage.removeItem(LAST_BG_KEY); } catch { /* ignore */ }
  resetStores();
  ui.cycleBackgrounds = true;
});
afterEach(() => { vi.useRealTimers(); vi.unstubAllGlobals(); });

/** What the engine must always have: a floor layer with a real image src, inside the engine, never display:none. */
function expectImage(root: ParentNode, url?: string): void {
  const engine = root.querySelector('[data-bg-engine]');
  expect(engine, 'engine mounted').toBeTruthy();
  const img = engine!.querySelector<HTMLImageElement>('[data-bg-floor] img.art');
  expect(img, 'floor image').toBeTruthy();
  expect(srcOf(img!)).toMatch(/\.(jpe?g|png|webp)/i);
  if (url) expect(srcOf(img!)).toBe(url);
  expect(getComputedStyle(img!).display).not.toBe('none');
  expect(img!.classList.contains('instant')).toBe(true);
  expect(engine!.querySelectorAll('.slot').length).toBeLessThanOrEqual(2);
}
async function run(target: HTMLElement, ms: number, step = 500): Promise<void> {
  for (let t = 0; t < ms; t += step) { await vi.advanceTimersByTimeAsync(step); expectImage(target); }
}
const mountShell = () => { const target = document.createElement('div'); document.body.append(target); const a = mount(Harness, { target }); flushSync(); return { target, a }; };

describe('the background is never missing', () => {
  it('before the app is ready (boot) and with zero instances', async () => {
    const { target, a } = mountShell();
    expectImage(target, floorUrl());           // first paint: no app state yet
    await init(createMockApi({ noProfiles: true }));
    flushSync();
    expect(app.profiles.length).toBe(0);
    expectImage(target, floorUrl());
    await run(target, 6_000);
    unmount(a);
  });

  it('every image fails except the bundled floor: the floor stays, nothing is retried forever', async () => {
    const fl = floorUrl();
    behave = (s) => (s === fl ? 'ok' : 'fail');
    await init(createMockApi());
    const { target, a } = mountShell();
    await run(target, 20_000);
    expectImage(target, fl);
    unmount(a);
  });

  it('even when every image, the floor included, fails there is still a floor layer with a real src and no crash', async () => {
    behave = () => 'fail';
    await init(createMockApi());
    const { target, a } = mountShell();
    await run(target, 10_000);
    unmount(a);
  });

  it('delayed decode: the floor is there while slides load, the slide fades in only once it loaded', async () => {
    const fl = floorUrl();
    behave = (s) => (s === fl ? 'ok' : 'delay');
    await init(createMockApi());
    const { target, a } = mountShell();
    await run(target, 5_000);
    expectImage(target, fl);
    // nothing but the floor is "ready" yet
    expect([...target.querySelectorAll<HTMLImageElement>('[data-bg-engine] .slot img.art.ready')].filter((i) => srcOf(i) !== fl).length).toBe(0);
    // now the slides arrive
    for (const img of target.querySelectorAll<HTMLImageElement>('[data-bg-engine] .slot img.art')) { ready.add(srcOf(img)); img.dispatchEvent(new Event('load')); }
    await run(target, 3_000);
    expect(target.querySelectorAll('[data-bg-engine] .slot img.art.ready').length).toBeGreaterThan(0);
    unmount(a);
  });

  it('the last shown bundled wallpaper paints first, a broken storage falls back to a fixed one', async () => {
    const other = wallpapers.find((x) => x.id !== floorWallpaper(wallpapers, null)!.id)!;
    localStorage.setItem(LAST_BG_KEY, other.id);
    await init(createMockApi());
    let m = mountShell();
    expectImage(m.target, other.url);
    unmount(m.a);
    vi.spyOn(Storage.prototype, 'getItem').mockImplementation(() => { throw new Error('blocked'); });
    m = mountShell();
    expectImage(m.target, floorUrl());
    unmount(m.a);
    vi.restoreAllMocks();
  });

  it('every view, the cycle switched off and reduced motion keep an image', async () => {
    ui.cycleBackgrounds = false;
    vi.stubGlobal('matchMedia', (q: string) => ({ matches: /reduce/.test(q), addEventListener() {}, removeEventListener() {} }));
    await init(createMockApi());
    const { target, a } = mountShell();
    const pid = app.profiles[0].id;
    for (const go of [() => { nav.view = 'library'; }, () => { nav.view = 'profile'; nav.profileId = pid; }, () => { nav.view = 'discover'; }, () => { nav.view = 'servers'; }, () => { nav.view = 'skins'; }, () => { nav.view = 'stats'; }, () => { nav.view = 'settings'; }, () => { nav.view = 'home'; }]) {
      go(); flushSync();
      await vi.advanceTimersByTimeAsync(200);
      expectImage(target);
    }
    unmount(a);
  });

  it('a deleted selected instance leaves the engine on an image', async () => {
    await init(createMockApi());
    const { target, a } = mountShell();
    await run(target, 2_000);
    app.profiles = []; if (app.config) app.config.selectedProfile = null;
    flushSync();
    await run(target, 4_000);
    unmount(a);
  });
});
