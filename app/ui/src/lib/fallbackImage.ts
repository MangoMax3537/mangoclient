// Single entry point for every bundled fallback image (CLAUDE.md image-first rule).
// The files are TEMPORARY placeholders, see src/assets/fallbacks/README.md.
export type FallbackKind = 'profile' | 'mod' | 'server' | 'account';

const files = import.meta.glob('../assets/fallbacks/*.png', {
  eager: true, query: '?url', import: 'default',
}) as Record<string, string>;

const COUNTS: Record<FallbackKind, number> = { profile: 16, mod: 3, server: 1, account: 1 };

/** FNV-1a, 32 bit. Stable across runs and platforms. */
export function hashId(id: string): number {
  let h = 0x811c9dc5;
  for (let i = 0; i < id.length; i++) {
    h ^= id.charCodeAt(i);
    h = Math.imul(h, 0x01000193) >>> 0;
  }
  return h >>> 0;
}

export function fallbackIndex(kind: FallbackKind, id: string): number {
  return kind === 'account' ? 0 : hashId(id) % COUNTS[kind];
}

function urlFor(stem: string): string {
  return files[`../assets/fallbacks/${stem}.png`] ?? '';
}

/** Deterministically map (kind, id) to a bundled image URL. */
export function fallbackImage(kind: FallbackKind, id: string): string {
  const n = fallbackIndex(kind, id);
  return urlFor(kind === 'account' ? 'placeholder-account-steve' : `placeholder-${kind}-${n}`);
}

export function heroFallback(id?: string): string {
  void id;
  return urlFor('placeholder-hero');
}

/** Skin head for an account: Microsoft accounts try the public head render, everything else (and any load error) uses the bundled head. */
export function accountHeadUrl(acc: { uuid: string; type: 'microsoft' | 'offline' } | null | undefined): string {
  if (acc && acc.type === 'microsoft') return `https://mc-heads.net/avatar/${encodeURIComponent(acc.uuid)}/64`;
  return fallbackImage('account', acc?.uuid ?? '');
}
