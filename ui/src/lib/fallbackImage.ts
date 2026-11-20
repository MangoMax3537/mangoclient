// Single entry point for every bundled fallback image (CLAUDE.md image-first rule).
// The only bundled fallback is the real MangoClient icon (src/assets/brand/). Real art can be
// dropped in later by swapping the lookup here; no caller needs to change.
export type FallbackKind = 'profile' | 'mod' | 'server' | 'account' | 'world';

const icons = import.meta.glob('../assets/brand/*x*.png', {
  eager: true, query: '?url', import: 'default',
}) as Record<string, string>;

/** FNV-1a, 32 bit. Stable across runs and platforms. */
export function hashId(id: string): number {
  let h = 0x811c9dc5;
  for (let i = 0; i < id.length; i++) {
    h ^= id.charCodeAt(i);
    h = Math.imul(h, 0x01000193) >>> 0;
  }
  return h >>> 0;
}

/** Number of neutral surface tones a tile can sit on (CSS only, the icon itself is never altered). */
export const SURFACE_TONES = 4;

/** Deterministic surface tone for (kind, id): 0..SURFACE_TONES-1. Accounts always use tone 0. */
export function fallbackIndex(kind: FallbackKind, id: string): number {
  return kind === 'account' ? 0 : hashId(id) % SURFACE_TONES;
}

const SIZES = [16, 24, 32, 48, 64, 128, 256, 512];

/** The MangoClient icon at the smallest bundled size that is at least `px` device pixels. */
export function brandIcon(px = 128): string {
  const s = SIZES.find((n) => n >= px) ?? 512;
  return icons[`../assets/brand/${s}x${s}.png`] ?? '';
}

/** Map (kind, id) to the bundled fallback image URL (the MangoClient icon). */
export function fallbackImage(kind: FallbackKind, id: string, px = 128): string {
  void kind; void id;
  return brandIcon(px);
}

/** Skin head for an account: the backend's `headUrl` (mangoimg://head/<uuid>); the MangoClient icon when absent. */
export function accountHeadUrl(acc: { headUrl?: string | null; uuid?: string } | null | undefined): string {
  return acc?.headUrl || fallbackImage('account', acc?.uuid ?? '');
}
