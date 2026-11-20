// Memory (RAM) per instance: pure helpers shared by the instance settings, the global settings, the plan dialogs and the mock.
// The backend owns the rules (docs/ipc.md Part 11); this file only places markers, picks warnings and debounces saves.
import type { RamHint, RamLimitsView, RamSource } from './types';

/** Slider step in MB. */
export const RAM_STEP = 256;
/** Above this share of the machine's memory the game starves the rest of the system. */
export const RAM_HIGH_SHARE = 0.75;
export const RAM_SAVE_DELAY = 450;

export interface RamEnv { limits: RamLimitsView; defaultMb: number; hint: RamHint | null }

/** The best single number a hint stands for: recommended, else max, else min (same order as the backend). */
export function hintTarget(h: RamHint | null | undefined): number | null {
  return h ? (h.recommendedMb ?? h.maxMb ?? h.minMb ?? null) : null;
}

/** Snap to the slider grid (steps of 256 MB counted from the minimum) and clamp into the limits. */
export function snapRam(mb: number, limits: RamLimitsView): number {
  if (!Number.isFinite(mb)) return limits.minMb;
  const steps = Math.round((mb - limits.minMb) / RAM_STEP);
  const v = limits.minMb + steps * RAM_STEP;
  const top = limits.minMb + Math.floor((limits.maxMb - limits.minMb) / RAM_STEP) * RAM_STEP;
  return Math.min(Math.max(v, limits.minMb), Math.max(top, limits.minMb));
}

/** Parse the GB field ("4", "4.5", "4,5") to MB; null when it is not a number. */
export function parseGb(text: string): number | null {
  const s = text.trim().replace(',', '.');
  if (!/^\d+(\.\d+)?$/.test(s)) return null;
  const n = Number(s);
  return Number.isFinite(n) ? Math.round(n * 1024) : null;
}

/** GB text for the numeric field: at most two decimals, no trailing zeros. */
export const gbText = (mb: number): string => String(+(mb / 1024).toFixed(2));

export type MarkerKind = 'default' | 'recommended' | 'cap' | 'min' | 'total';
export interface Marker { kind: MarkerKind; mb: number; pct: number; /** true when the value lies outside the track and is pinned to its end */ clamped: boolean }

/** Position (0..100) of `mb` on a track from `min` to `max`; clamped, with a flag when it was outside. */
export function trackPct(mb: number, min: number, max: number): { pct: number; clamped: boolean } {
  if (max <= min) return { pct: 0, clamped: mb !== min };
  const raw = ((mb - min) / (max - min)) * 100;
  return { pct: Math.min(100, Math.max(0, raw)), clamped: raw < 0 || raw > 100 };
}

/** Markers on the slider track: the launcher default, the pack's recommendation / minimum / cap, and the machine's memory. */
export function ramMarkers(env: RamEnv): Marker[] {
  const { limits, hint, defaultMb } = env;
  const out: Marker[] = [];
  const add = (kind: MarkerKind, mb: number | null | undefined) => {
    if (mb == null) return;
    out.push({ kind, mb, ...trackPct(mb, limits.minMb, limits.maxMb) });
  };
  add('default', defaultMb);
  add('recommended', hint?.recommendedMb);
  add('min', hint?.minMb);
  add('cap', hint?.maxMb);
  add('total', limits.totalMb);
  return out;
}

export type RamWarningKind = 'cap' | 'high' | 'min' | 'limit';
export interface RamWarning { kind: RamWarningKind; /** value the message names, MB */ mb: number }

/** Warnings for a chosen value. Order = importance. */
export function ramWarnings(mb: number, env: RamEnv): RamWarning[] {
  const { limits, hint } = env;
  const out: RamWarning[] = [];
  if (hint?.maxMb != null && mb > hint.maxMb) out.push({ kind: 'cap', mb: hint.maxMb });
  if (mb >= limits.maxMb) out.push({ kind: 'limit', mb: limits.maxMb });
  else if (mb > limits.totalMb * RAM_HIGH_SHARE) out.push({ kind: 'high', mb: Math.round(limits.totalMb * RAM_HIGH_SHARE) });
  if (hint?.minMb != null && mb < hint.minMb) out.push({ kind: 'min', mb: hint.minMb });
  return out;
}

export const ramSourceKey = (s: RamSource) => (s === 'modpack' ? 'ram.source.modpack' : s === 'user' ? 'ram.source.user' : 'ram.source.default') as
  'ram.source.modpack' | 'ram.source.user' | 'ram.source.default';

/** What the two reset buttons offer. */
export function ramResets(value: number, source: RamSource, env: RamEnv): { canDefault: boolean; packMb: number | null; canPack: boolean } {
  const packMb = hintTarget(env.hint);
  const pack = packMb === null ? null : snapRam(packMb, env.limits);
  return {
    canDefault: source !== 'default',
    packMb: pack,
    canPack: pack !== null && !(source === 'modpack' && value === pack),
  };
}

/** Debounced saver: `set` schedules a save of the latest value, `flush` saves now, nothing is sent when the value equals what is stored. */
export class RamSaver {
  private timer: ReturnType<typeof setTimeout> | undefined;
  private pending: number | null | undefined;
  constructor(private save: (ram: number | null) => void, private current: () => number | null, private delay = RAM_SAVE_DELAY) {}
  set(v: number | null) {
    this.pending = v;
    clearTimeout(this.timer);
    this.timer = setTimeout(() => this.flush(), this.delay);
  }
  flush() {
    clearTimeout(this.timer);
    const v = this.pending;
    this.pending = undefined;
    if (v !== undefined && v !== this.current()) this.save(v);
  }
  cancel() { clearTimeout(this.timer); this.pending = undefined; }
  get dirty() { return this.pending !== undefined; }
}

/** What a new instance gets from a plan: the applied heap when there is one, else the launcher default. */
export function planRam(plan: { ram: RamHint | null; appliedRamMb: number | null }, defaultMb: number): { mb: number; fromPack: boolean; capMb: number | null } {
  const fromPack = plan.appliedRamMb != null;
  return { mb: plan.appliedRamMb ?? defaultMb, fromPack, capMb: plan.ram?.maxMb ?? null };
}

/** The mock's copy of the backend's derived view fields. */
export function derivedRam(ram: number | null, source: RamSource | undefined, configRam: number): { ramSource: RamSource; effectiveRamMb: number } {
  if (ram === null) return { ramSource: 'default', effectiveRamMb: configRam };
  return { ramSource: source === 'modpack' ? 'modpack' : 'user', effectiveRamMb: ram };
}
