import { describe, expect, it, vi } from 'vitest';
import { imgPhase, imgWatch, showsFallback } from './imgLoad';

describe('image slot phases', () => {
  it('no URL is its own state and shows the fallback straight away', () => {
    expect(imgPhase(null, false, false)).toBe('none');
    expect(imgPhase('', false, false)).toBe('none');
    expect(imgPhase(undefined, true, false)).toBe('none');
    expect(showsFallback('none')).toBe(true);
  });
  it('loading is a plain grey box (no fallback yet), loaded shows the image, error the fallback', () => {
    expect(imgPhase('a.png', false, false)).toBe('loading');
    expect(showsFallback('loading')).toBe(false);
    expect(imgPhase('a.png', true, false)).toBe('loaded');
    expect(showsFallback('loaded')).toBe(false);
    expect(imgPhase('a.png', false, true)).toBe('error');
    expect(imgPhase('a.png', true, true)).toBe('error');
    expect(showsFallback('error')).toBe(true);
  });
});

function fakeImg(opts: { complete: boolean; naturalWidth?: number; src?: string }) {
  const t = new EventTarget() as EventTarget & { complete: boolean; naturalWidth: number; getAttribute: (n: string) => string | null };
  t.complete = opts.complete; t.naturalWidth = opts.naturalWidth ?? 0; t.getAttribute = () => opts.src ?? 'x.png';
  return t as unknown as HTMLImageElement & EventTarget;
}

describe('imgWatch', () => {
  it('reports load and error events', () => {
    const img = fakeImg({ complete: false });
    const onload = vi.fn(); const onerror = vi.fn();
    const a = imgWatch(img, { onload, onerror, src: 'x.png' });
    img.dispatchEvent(new Event('load'));
    expect(onload).toHaveBeenCalledWith(false);
    img.dispatchEvent(new Event('error'));
    expect(onerror).toHaveBeenCalledTimes(1);
    a.destroy();
    img.dispatchEvent(new Event('load'));
    expect(onload).toHaveBeenCalledTimes(1);
  });
  it('an image that finished before the action ran counts as loaded instantly (no fade), a broken one as error', () => {
    const ok = fakeImg({ complete: true, naturalWidth: 64 });
    const onload = vi.fn();
    imgWatch(ok, { onload, onerror: vi.fn() });
    expect(onload).toHaveBeenCalledWith(true);
    const bad = fakeImg({ complete: true, naturalWidth: 0 });
    const onerror = vi.fn();
    imgWatch(bad, { onload: vi.fn(), onerror });
    expect(onerror).toHaveBeenCalled();
  });
  it('an unfinished image reports nothing until its event', () => {
    const img = fakeImg({ complete: false });
    const onload = vi.fn(); const onerror = vi.fn();
    imgWatch(img, { onload, onerror });
    expect(onload).not.toHaveBeenCalled();
    expect(onerror).not.toHaveBeenCalled();
  });
});

import { decodeSoon } from './imgLoad';
describe('decodeSoon', () => {
  it('returns once decode settles', async () => { await expect(decodeSoon({ decode: () => Promise.resolve() })).resolves.toBeUndefined(); });
  it('a decode that rejects still counts as done', async () => { await expect(decodeSoon({ decode: () => Promise.reject(new Error('x')) })).resolves.toBeUndefined(); });
  it('a decode that never settles (WebKitGTK) is cut off after the ceiling', async () => {
    const t0 = Date.now();
    await decodeSoon({ decode: () => new Promise<void>(() => {}) }, 40);
    expect(Date.now() - t0).toBeLessThan(500);
  });
  it('no element / no decode() is fine', async () => { await decodeSoon(null); await decodeSoon({} as never); });
});
