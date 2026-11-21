// Which game versions a server accepts, and whether an instance fits. The range itself is parsed in Rust (`mango_servers::version`, from the ping's
// version name or protocol); this file only compares and formats it. A `max` ending in `.x` means "any of that line" (26.x).
import type { VersionRange } from './types';

export type Compat = 'yes' | 'no' | 'unknown';

const NUM = /^\d+(\.\d+){0,2}$/;
/** `1.21.4` -> [1,21,4]; anything else (snapshots, `24w14a`) -> null. */
function parts(v: string): number[] | null {
  return NUM.test(v) ? v.split('.').map(Number) : null;
}
const cmp = (a: number[], b: number[]): number => {
  for (let i = 0; i < Math.max(a.length, b.length); i++) { const d = (a[i] ?? 0) - (b[i] ?? 0); if (d) return d < 0 ? -1 : 1; }
  return 0;
};

/** Is `mc` inside the range? `unknown` without a range or for a version that is not a plain release number. */
export function compatibility(range: VersionRange | null | undefined, mc: string): Compat {
  if (!range) return 'unknown';
  const v = parts(mc);
  const lo = parts(range.min);
  if (!v || !lo) return 'unknown';
  const wild = range.max.endsWith('.x');
  const hi = parts(wild ? range.max.slice(0, -2) : range.max);
  if (!hi) return 'unknown';
  if (cmp(v, lo) < 0) return 'no';
  // a wildcard bound covers the whole line: compare only the segments it names
  const upper = wild ? v.slice(0, hi.length) : v;
  return cmp(upper, hi) <= 0 ? 'yes' : 'no';
}

/** `1.21.11 - 26.3`, or a single version when both ends are the same; null without a range. */
export function rangeLabel(range: VersionRange | null | undefined): string | null {
  if (!range) return null;
  return range.min === range.max || `${range.min}.x` === range.max ? range.max : `${range.min} - ${range.max}`;
}
