// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { flushSync, mount, unmount } from 'svelte';
import Harness from './testing/BgHarness.svelte';
import { createMockApi } from '../lib/mockApi';
import { init, resetStores, ui } from '../lib/stores.svelte';

// jsdom never loads images: report every <img> as loaded + decodable so the engine can run its swaps
beforeEach(() => {
  Object.defineProperty(HTMLImageElement.prototype, 'complete', { configurable: true, get: () => true });
  Object.defineProperty(HTMLImageElement.prototype, 'naturalWidth', { configurable: true, get: () => 64 });
  (HTMLImageElement.prototype as unknown as { decode: () => Promise<void> }).decode = () => Promise.resolve();
  vi.stubGlobal('requestAnimationFrame', (f: FrameRequestCallback) => setTimeout(() => f(0), 16));
  vi.stubGlobal('matchMedia', () => ({ matches: false, addEventListener() {}, removeEventListener() {} }));
  (HTMLCanvasElement.prototype as unknown as { getContext: () => unknown }).getContext = () => new Proxy({}, { get: () => () => undefined, set: () => true });
  (HTMLCanvasElement.prototype as unknown as { toDataURL: () => string }).toDataURL = () => 'data:image/png;base64,';
  vi.useFakeTimers();
  history.replaceState(null, '', '/?mockCycleMs=1500');
  resetStores();
});
afterEach(() => { vi.useRealTimers(); vi.unstubAllGlobals(); });

const srcs = (root: ParentNode) => [...root.querySelectorAll<HTMLImageElement>('[data-bg-engine] img')].map((i) => i.getAttribute('src'));

describe('background engine', () => {
  it('cycles on and on (8 swaps), and the hero content nodes are never touched', async () => {
    await init(createMockApi());
    const target = document.createElement('div'); document.body.append(target);
    const app = mount(Harness, { target });
    flushSync();
    await vi.advanceTimersByTimeAsync(50);
    const h1 = target.querySelector('h1')!, info = target.querySelector('.facts')!, content = target.querySelector('.content')!;
    expect(h1).toBeTruthy(); expect(info.textContent).toMatch(/\//);
    // the engine layer sits in its own stacking context, content above it
    expect(target.querySelector('[data-bg-engine]')).toBeTruthy();
    expect(target.querySelector('[data-bg-engine]')?.contains(h1)).toBe(false);

    const seen = new Set<string>();
    let swaps = 0, last = JSON.stringify(srcs(target));
    for (let i = 0; i < 40 && swaps < 8; i++) {
      await vi.advanceTimersByTimeAsync(500);
      const cur = JSON.stringify(srcs(target));
      if (cur !== last) { last = cur; swaps++; seen.add(cur); }
      // same DOM nodes, still in the document, never hidden
      expect(target.querySelector('h1')).toBe(h1);
      expect(target.querySelector('.facts')).toBe(info);
      expect(target.querySelector('.content')).toBe(content);
      expect(h1.isConnected && info.isConnected).toBe(true);
      expect(h1.textContent).toBeTruthy();
      expect(getComputedStyle(h1).visibility).not.toBe('hidden');
      expect(getComputedStyle(content).display).not.toBe('none');
    }
    expect(swaps).toBeGreaterThanOrEqual(8);
    expect(seen.size).toBeGreaterThan(2);
    // two layers at most: the outgoing images are dropped
    expect(target.querySelectorAll('[data-bg-engine] .slot').length).toBeLessThanOrEqual(2);
    unmount(app);
  });

  it('does not cycle when the preference is off', async () => {
    ui.cycleBackgrounds = false;
    await init(createMockApi());
    const target = document.createElement('div'); document.body.append(target);
    const app = mount(Harness, { target });
    flushSync();
    await vi.advanceTimersByTimeAsync(50);
    const first = JSON.stringify(srcs(target));
    await vi.advanceTimersByTimeAsync(20_000);
    expect(JSON.stringify(srcs(target))).toBe(first);
    unmount(app);
  });
});
