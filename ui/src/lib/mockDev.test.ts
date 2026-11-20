import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { DEFAULT_MOCK_LATENCY_MS, mockDevFromSearch, withDevControls } from './mockDev';

describe('mock dev controls', () => {
  it('parses latency, per-command delays (UI or Rust names), slow and fail lists', () => {
    expect(mockDevFromSearch('').base).toBe(DEFAULT_MOCK_LATENCY_MS);
    const o = mockDevFromSearch('?mockLatency=1500&mockDelay=search_projects:4000,statsSummary:800&mockSlow=searchProjects,servers_ping*&mockFail=statsSummary');
    expect(o.base).toBe(1500);
    expect(o.perCommand).toEqual({ searchProjects: 4000, statsSummary: 800 });
    expect(o.slow).toEqual([{ name: 'searchProjects', always: false }, { name: 'serversPing', always: true }]);
    expect(o.fail).toEqual([{ name: 'statsSummary', always: false }]);
  });
  it('the default latency stays below the 150 ms skeleton delay', () => { expect(DEFAULT_MOCK_LATENCY_MS).toBeLessThan(150); });

  describe('wrapping', () => {
    beforeEach(() => { vi.useFakeTimers(); });
    afterEach(() => { vi.useRealTimers(); });
    const api = { async a() { return 1; }, async b() { return 2; }, listen: () => 'untouched', dev: { x: 1 } };
    it('delays every command except listen', async () => {
      const w = withDevControls(api, { base: 100, perCommand: { b: 500 }, slow: [], fail: [] });
      let a = 0; void w.a().then((v) => { a = v; });
      await vi.advanceTimersByTimeAsync(99);
      expect(a).toBe(0);
      await vi.advanceTimersByTimeAsync(2);
      expect(a).toBe(1);
      expect(w.listen()).toBe('untouched');
      expect(w.dev).toBe(api.dev);
      let b = 0; void w.b().then((v) => { b = v; });
      await vi.advanceTimersByTimeAsync(400);
      expect(b).toBe(0);
      await vi.advanceTimersByTimeAsync(101);
      expect(b).toBe(2);
    });
    it('slow hangs the first call only; fail fails the first call only; `*` makes it permanent', async () => {
      const w = withDevControls(api, { base: 0, perCommand: {}, slow: [{ name: 'a', always: false }], fail: [{ name: 'b', always: false }] });
      let done = false; void w.a().then(() => { done = true; });
      await vi.advanceTimersByTimeAsync(60_000);
      expect(done).toBe(false);
      await expect(w.a()).resolves.toBe(1);
      await expect(w.b()).rejects.toMatchObject({ code: 'net.request' });
      await expect(w.b()).resolves.toBe(2);
      const f = withDevControls(api, { base: 0, perCommand: {}, slow: [], fail: [{ name: 'b', always: true }] });
      await expect(f.b()).rejects.toBeTruthy();
      await expect(f.b()).rejects.toBeTruthy();
    });
  });
});
