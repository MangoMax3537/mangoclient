import { describe, expect, it } from 'vitest';
import { NAME_MAX, checkName, problemKey } from './rename';
import { hasKey } from './i18n';

describe('checkName (same rules as Rust)', () => {
  it('trims and accepts', () => { expect(checkName('  Iron Hollow ')).toEqual({ ok: true, name: 'Iron Hollow' }); });
  it('rejects empty after trim', () => { expect(checkName('   ')).toEqual({ ok: false, problem: 'empty' }); expect(checkName('')).toEqual({ ok: false, problem: 'empty' }); });
  it('128 characters (not bytes) is the limit', () => {
    expect(checkName('a'.repeat(NAME_MAX)).ok).toBe(true);
    expect(checkName('a'.repeat(NAME_MAX + 1))).toEqual({ ok: false, problem: 'long' });
    expect(checkName('é'.repeat(NAME_MAX)).ok).toBe(true);
  });
  it('rejects control characters', () => {
    for (const c of ['a\u0000b', 'a\nb', 'a\tb', 'a\u007fb', 'a b']) expect(checkName(c), JSON.stringify(c)).toEqual({ ok: false, problem: 'control' });
  });
  it('every problem has a message in both languages', () => { for (const p of ['empty', 'long', 'control'] as const) { expect(hasKey('en', problemKey(p))).toBe(true); expect(hasKey('de', problemKey(p))).toBe(true); } });
});
