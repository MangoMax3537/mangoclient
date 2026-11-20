import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { derivedRam, gbText, hintTarget, parseGb, planRam, RamSaver, ramMarkers, ramResets, ramSourceKey, ramWarnings, snapRam, trackPct, type RamEnv } from './ram';
import { dictionaries, hasKey } from './i18n';

const limits = { minMb: 1024, maxMb: 14336, totalMb: 16384 };
const hint = { recommendedMb: 4096, minMb: 3072, maxMb: 6144, source: 'modrinth' as const };
const env: RamEnv = { limits, defaultMb: 4096, hint };
const noHint: RamEnv = { limits, defaultMb: 4096, hint: null };

describe('markers', () => {
  it('places default, pack recommendation, minimum, cap and machine total on the track', () => {
    const m = Object.fromEntries(ramMarkers(env).map((x) => [x.kind, x]));
    expect(m.default.pct).toBeCloseTo(((4096 - 1024) / 13312) * 100, 5);
    expect(m.recommended.mb).toBe(4096);
    expect(m.min.mb).toBe(3072);
    expect(m.cap.pct).toBeCloseTo(((6144 - 1024) / 13312) * 100, 5);
    expect(m.default.clamped).toBe(false);
  });
  it('the machine total lies past the end of the track and is pinned to it', () => {
    const t = ramMarkers(env).find((x) => x.kind === 'total')!;
    expect(t.pct).toBe(100);
    expect(t.clamped).toBe(true);
  });
  it('without a hint only default and total are shown', () => {
    expect(ramMarkers(noHint).map((x) => x.kind)).toEqual(['default', 'total']);
  });
  it('trackPct clamps and survives a degenerate track', () => {
    expect(trackPct(500, 1024, 2048)).toEqual({ pct: 0, clamped: true });
    expect(trackPct(1536, 1024, 2048)).toEqual({ pct: 50, clamped: false });
    expect(trackPct(1, 5, 5).pct).toBe(0);
  });
});

describe('warnings', () => {
  const kinds = (mb: number, e = env) => ramWarnings(mb, e).map((w) => w.kind);
  it('none inside the pack range', () => { expect(kinds(4096)).toEqual([]); expect(kinds(6144)).toEqual([]); });
  it('above the pack cap names the cap', () => {
    expect(ramWarnings(6400, env)).toEqual([{ kind: 'cap', mb: 6144 }]);
    expect(kinds(6400, noHint)).toEqual([]);
  });
  it('above 75 percent of the machine memory', () => {
    expect(kinds(12288, noHint)).toEqual([]); // exactly 75 percent is fine
    expect(kinds(12544, noHint)).toEqual(['high']);
  });
  it('below the pack minimum', () => { expect(kinds(2048)).toEqual(['min']); expect(kinds(3072)).toEqual([]); });
  it('at the hard limit replaces the 75 percent warning', () => {
    expect(kinds(14336, noHint)).toEqual(['limit']);
    expect(kinds(14336)).toEqual(['cap', 'limit']);
  });
  it('every warning has text in both languages', () => {
    for (const k of ['cap', 'high', 'min', 'limit']) { expect(hasKey('en', `ram.warn.${k}`)).toBe(true); expect(hasKey('de', `ram.warn.${k}`)).toBe(true); }
    expect(dictionaries.en['ram.warn.cap']).toMatch(/garbage collector/);
  });
});

describe('source label and resets', () => {
  it('maps the three sources to their labels', () => {
    expect(dictionaries.en[ramSourceKey('default')]).toBe('Default');
    expect(dictionaries.en[ramSourceKey('modpack')]).toBe('From modpack');
    expect(dictionaries.en[ramSourceKey('user')]).toBe('Set by you');
  });
  it('"Use default" is only offered when the value is not already the default', () => {
    expect(ramResets(4096, 'default', env).canDefault).toBe(false);
    expect(ramResets(6144, 'user', env).canDefault).toBe(true);
  });
  it('"Use pack recommendation" needs a hint and is off while it is already applied', () => {
    expect(ramResets(6144, 'user', noHint)).toMatchObject({ canPack: false, packMb: null });
    expect(ramResets(6144, 'user', env)).toMatchObject({ canPack: true, packMb: 4096 });
    expect(ramResets(4096, 'modpack', env).canPack).toBe(false);
  });
  it('hintTarget prefers recommended, then max, then min', () => {
    expect(hintTarget(hint)).toBe(4096);
    expect(hintTarget({ ...hint, recommendedMb: null })).toBe(6144);
    expect(hintTarget({ recommendedMb: null, maxMb: null, minMb: 2048, source: 'prism' })).toBe(2048);
    expect(hintTarget(null)).toBeNull();
  });
});

describe('slider and GB field', () => {
  it('snaps to 256 MB steps from the minimum, inside the limits', () => {
    expect(snapRam(4100, limits)).toBe(4096);
    expect(snapRam(4300, limits)).toBe(4352);
    expect(snapRam(10, limits)).toBe(1024);
    expect(snapRam(99999, limits)).toBe(14336);
    expect(snapRam(NaN, limits)).toBe(1024);
  });
  it('parses GB with dot or comma and rejects the rest', () => {
    expect(parseGb('4')).toBe(4096);
    expect(parseGb('4,5')).toBe(4608);
    expect(parseGb(' 0.25 ')).toBe(256);
    for (const bad of ['', 'abc', '-1', '4 GB', '1e3']) expect(parseGb(bad)).toBeNull();
  });
  it('formats MB as GB without trailing zeros', () => { expect(gbText(4096)).toBe('4'); expect(gbText(4608)).toBe('4.5'); expect(gbText(2816)).toBe('2.75'); });
});

describe('debounced saver', () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());
  it('saves only the latest value, once, after the delay', () => {
    const save = vi.fn();
    const s = new RamSaver(save, () => 4096, 450);
    s.set(5120); s.set(5376); s.set(6144);
    vi.advanceTimersByTime(449);
    expect(save).not.toHaveBeenCalled();
    vi.advanceTimersByTime(2);
    expect(save).toHaveBeenCalledOnce();
    expect(save).toHaveBeenCalledWith(6144);
  });
  it('sends nothing when the value ends up unchanged', () => {
    const save = vi.fn();
    const s = new RamSaver(save, () => 4096, 450);
    s.set(5120); s.set(4096);
    vi.advanceTimersByTime(1000);
    expect(save).not.toHaveBeenCalled();
  });
  it('flush saves now, cancel drops the pending value', () => {
    const save = vi.fn();
    const s = new RamSaver(save, () => 4096, 450);
    s.set(5120); expect(s.dirty).toBe(true); s.flush();
    expect(save).toHaveBeenCalledWith(5120); expect(s.dirty).toBe(false);
    s.set(6144); s.cancel(); vi.advanceTimersByTime(1000);
    expect(save).toHaveBeenCalledOnce();
  });
});

describe('plan RAM line and mock view fields', () => {
  it('uses the applied pack value, else the default, and carries the cap', () => {
    expect(planRam({ ram: hint, appliedRamMb: 4096 }, 5120)).toEqual({ mb: 4096, fromPack: true, capMb: 6144 });
    expect(planRam({ ram: null, appliedRamMb: null }, 5120)).toEqual({ mb: 5120, fromPack: false, capMb: null });
  });
  it('derivedRam follows the backend rules', () => {
    expect(derivedRam(null, 'user', 4096)).toEqual({ ramSource: 'default', effectiveRamMb: 4096 });
    expect(derivedRam(6144, 'modpack', 4096)).toEqual({ ramSource: 'modpack', effectiveRamMb: 6144 });
    expect(derivedRam(6144, 'default', 4096).ramSource).toBe('user');
  });
});
