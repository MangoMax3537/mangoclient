// Instance name validation, the same rules as the Rust side (`validation.rs`: `text_str(.., "profile name", 128, false)` then trim):
// non-empty after trimming, at most 128 characters (not bytes), no control characters. The UI shows the problem inline before anything is sent.
import type { Key } from './i18n';

export const NAME_MAX = 128;
export type NameProblem = 'empty' | 'long' | 'control';
export type NameCheck = { ok: true; name: string } | { ok: false; problem: NameProblem };

const CONTROL = new RegExp('[\\u0000-\\u001f\\u007f-\\u009f\\u2028\\u2029]');

export function checkName(raw: string): NameCheck {
  if (CONTROL.test(raw)) return { ok: false, problem: 'control' };
  const name = raw.trim();
  if (!name) return { ok: false, problem: 'empty' };
  if ([...name].length > NAME_MAX) return { ok: false, problem: 'long' };
  return { ok: true, name };
}
export const problemKey = (p: NameProblem): Key => (p === 'empty' ? 'rename.err.empty' : p === 'long' ? 'rename.err.long' : 'rename.err.control');
