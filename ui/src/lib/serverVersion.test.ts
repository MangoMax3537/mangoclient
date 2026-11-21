import { describe, expect, it } from 'vitest';
import { compatibility, rangeLabel } from './serverVersion';

const r = (min: string, max: string) => ({ min, max });
describe('server version compatibility', () => {
  it('inside, below and above a concrete range', () => {
    const x = r('1.21.5', '26.3');
    expect(compatibility(x, '1.21.5')).toBe('yes');
    expect(compatibility(x, '1.21.11')).toBe('yes');
    expect(compatibility(x, '26.3')).toBe('yes');
    expect(compatibility(x, '1.21.4')).toBe('no');
    expect(compatibility(x, '26.3.1')).toBe('no');
    expect(compatibility(x, '1.8.9')).toBe('no');
  });
  it('compares numerically, not as text (1.9 < 1.21)', () => {
    expect(compatibility(r('1.9', '1.21'), '1.12.2')).toBe('yes');
    expect(compatibility(r('1.21', '1.21'), '1.9')).toBe('no');
  });
  it('a .x upper bound covers the whole line', () => {
    expect(compatibility(r('1.21.5', '26.x'), '26.3')).toBe('yes');
    expect(compatibility(r('1.21.5', '26.x'), '26.1.2')).toBe('yes');
    expect(compatibility(r('1.21.5', '26.x'), '27.1')).toBe('no');
    expect(compatibility(r('1.21', '1.21.x'), '1.21.11')).toBe('yes');
    expect(compatibility(r('1.21', '1.21.x'), '1.22')).toBe('no');
  });
  it('unknown without a range or for versions that are not release numbers', () => {
    expect(compatibility(null, '1.21.4')).toBe('unknown');
    expect(compatibility(undefined, '1.21.4')).toBe('unknown');
    expect(compatibility(r('1.21', '1.21'), '24w14a')).toBe('unknown');
    expect(compatibility(r('1.21', '1.21'), '')).toBe('unknown');
  });
  it('labels', () => {
    expect(rangeLabel(r('1.21.11', '26.3'))).toBe('1.21.11 - 26.3');
    expect(rangeLabel(r('1.21.5', '26.x'))).toBe('1.21.5 - 26.x');
    expect(rangeLabel(r('1.21.4', '1.21.4'))).toBe('1.21.4');
    expect(rangeLabel(r('1.21', '1.21.x'))).toBe('1.21.x');
    expect(rangeLabel(null)).toBeNull();
  });
});
