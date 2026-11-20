// Per-instance Java and window settings: the pure rules the form uses (docs/ipc.md Part 13), mirroring the Rust validation.
import type { JavaInfo, WindowSettings } from './types';

/** Java major a Minecraft version needs (mirror of the launcher's fallback; the version manifest may declare more). */
export function requiredJavaMajor(mc: string): number | null {
  if (/^\d{2}\.\d+/.test(mc)) return 25;
  const m = /^1\.(\d+)(?:\.(\d+))?/.exec(mc);
  if (m) {
    const minor = Number(m[1]); const patch = Number(m[2] ?? 0);
    if (minor >= 21) return 21;
    if (minor === 20) return patch >= 5 ? 21 : 17;
    if (minor >= 18) return 17;
    if (minor === 17) return 16;
    return 8;
  }
  const snap = /^(\d{2})w(\d{2})[a-z]$/.exec(mc);
  if (snap) { const y = Number(snap[1]); const w = Number(snap[2]); return y > 24 || (y === 24 && w >= 14) ? 21 : y >= 21 ? 17 : null; }
  return null;
}

export const javaLabel = (j: JavaInfo): string => `Java ${j.major}${j.vendor ? ` (${j.vendor}${j.source === 'managed' ? ', managed' : ''})` : j.source === 'managed' ? ' (managed)' : ''}`;
/** True when no known runtime of that major exists (managed, system or custom). */
export const missingMajor = (list: readonly JavaInfo[], major: number | null): boolean => major !== null && !list.some((j) => j.major === major);

export const WIN_W = { min: 640, max: 16384 } as const;
export const WIN_H = { min: 480, max: 16384 } as const;
export const WINDOW_PRESETS: readonly (readonly [number, number])[] = [[854, 480], [1280, 720], [1600, 900], [1920, 1080]];

const whole = (s: string): number | null => (/^\s*\d{1,5}\s*$/.test(s) ? Number(s) : null);
export interface SizeCheck { ok: boolean; width: boolean; height: boolean; value?: { width: number; height: number } }
/** Same bounds as Rust: width 640..16384, height 480..16384, whole numbers, never clamped. */
export function checkInstanceSize(w: string, h: string): SizeCheck {
  const width = whole(w); const height = whole(h);
  const wOk = width !== null && width >= WIN_W.min && width <= WIN_W.max;
  const hOk = height !== null && height >= WIN_H.min && height <= WIN_H.max;
  return { ok: wOk && hOk, width: wOk, height: hOk, ...(wOk && hOk ? { value: { width: width as number, height: height as number } } : {}) };
}

/** The `window` patch for a mode (default and fullscreen carry no size). */
export function windowPatch(mode: WindowSettings['mode'], size?: { width: number; height: number }): WindowSettings {
  return mode === 'size' && size ? { mode, width: size.width, height: size.height } : { mode, width: null, height: null };
}

/** A path typed into the custom Java field: blank = automatic (`null`), otherwise trimmed. */
export const normalizeJavaPath = (s: string): string | null => (s.trim() ? s.trim() : null);
