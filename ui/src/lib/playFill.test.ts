import { describe, expect, it } from 'vitest';
import { fillClip } from './playFill';

describe('fillClip', () => {
  it('maps percent to complementary insets', () => {
    const c = fillClip(0.53);
    expect(c.pct).toBe(53);
    expect(c.fillInset).toBe('inset(0 47% 0 0)');
    expect(c.trackInset).toBe('inset(0 0 0 53%)');
  });
  it('0 and 100 cover nothing / everything', () => {
    expect(fillClip(0).fillInset).toBe('inset(0 100% 0 0)');
    expect(fillClip(1).fillInset).toBe('inset(0 0% 0 0)');
    expect(fillClip(1).trackInset).toBe('inset(0 0 0 100%)');
  });
  it('clamps out-of-range and NaN', () => {
    expect(fillClip(-3).pct).toBe(0);
    expect(fillClip(7).pct).toBe(100);
    expect(fillClip(Number.NaN).pct).toBe(0);
  });
  it('null is indeterminate', () => {
    expect(fillClip(null).indeterminate).toBe(true);
    expect(fillClip(undefined).indeterminate).toBe(true);
    expect(fillClip(0.2).indeterminate).toBe(false);
  });
});
