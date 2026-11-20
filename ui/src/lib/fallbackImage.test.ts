import { describe, expect, it } from 'vitest';
import { SURFACE_TONES, accountHeadUrl, brandIcon, fallbackImage, fallbackIndex, hashId } from './fallbackImage';

describe('fallbackImage', () => {
  it('is deterministic', () => {
    const id = '3f1c2a7e-5b1d-4c0a-9d51-0a6f5e2b7c11';
    expect(fallbackImage('profile', id)).toBe(fallbackImage('profile', id));
    expect(hashId(id)).toBe(hashId(id));
  });
  it('returns a non-empty url per kind (the MangoClient icon)', () => {
    for (const k of ['profile', 'mod', 'server', 'account'] as const) expect(fallbackImage(k, 'x')).toBeTruthy();
  });
  it('picks the smallest bundled size that is large enough', () => {
    expect(brandIcon(10)).toContain('16x16');
    expect(brandIcon(100)).toContain('128x128');
    expect(brandIcon(9999)).toContain('512x512');
  });
  it('spreads ids over the surface tones within range', () => {
    const seen = new Set<number>();
    for (let i = 0; i < 200; i++) {
      const n = fallbackIndex('profile', `id-${i}`);
      expect(n).toBeGreaterThanOrEqual(0);
      expect(n).toBeLessThan(SURFACE_TONES);
      seen.add(n);
    }
    expect(seen.size).toBe(SURFACE_TONES);
  });
  it('accounts always use the same tone', () => {
    expect(fallbackIndex('account', 'a')).toBe(fallbackIndex('account', 'b'));
  });
  it('account heads come from the backend headUrl, never a third-party host', () => {
    expect(accountHeadUrl({ uuid: 'u', headUrl: 'mangoimg://head/u' })).toBe('mangoimg://head/u');
    expect(accountHeadUrl({ uuid: 'u', headUrl: '' })).toBe(fallbackImage('account', 'u'));
    expect(accountHeadUrl(null)).toBe(fallbackImage('account', ''));
  });
});
