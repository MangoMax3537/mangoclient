import { describe, expect, it } from 'vitest';
import { accountHeadUrl, fallbackImage, fallbackIndex, hashId } from './fallbackImage';

describe('fallbackImage', () => {
  it('is deterministic', () => {
    const id = '3f1c2a7e-5b1d-4c0a-9d51-0a6f5e2b7c11';
    expect(fallbackImage('profile', id)).toBe(fallbackImage('profile', id));
    expect(hashId(id)).toBe(hashId(id));
  });
  it('returns a non-empty url per kind', () => {
    for (const k of ['profile', 'mod', 'server', 'account'] as const) expect(fallbackImage(k, 'x')).toBeTruthy();
  });
  it('spreads profile ids over the 16 tiles within range', () => {
    const seen = new Set<number>();
    for (let i = 0; i < 400; i++) {
      const n = fallbackIndex('profile', `id-${i}`);
      expect(n).toBeGreaterThanOrEqual(0);
      expect(n).toBeLessThan(16);
      seen.add(n);
    }
    expect(seen.size).toBe(16);
  });
  it('accounts always use the bundled head', () => {
    expect(fallbackImage('account', 'a')).toBe(fallbackImage('account', 'b'));
  });
  it('account heads come from the backend headUrl, never a third-party host', () => {
    expect(accountHeadUrl({ uuid: 'u', headUrl: 'mangoimg://head/u' })).toBe('mangoimg://head/u');
    expect(accountHeadUrl({ uuid: 'u', headUrl: '' })).toBe(fallbackImage('account', 'u'));
    expect(accountHeadUrl(null)).toBe(fallbackImage('account', ''));
  });
});
