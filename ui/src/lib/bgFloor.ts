// The guaranteed floor of every background: a wallpaper that ships inside the app. Whatever else fails (version wallpaper, panorama,
// custom image, cover, an empty pool, a slow or broken decode) this one is drawn, so the window is never a flat colour.
import { hashId } from './fallbackImage';
import type { Wallpaper } from './wallpapers';

export const LAST_BG_KEY = 'mango-bg-last';

/** The id of the bundled wallpaper that was on screen last time, if remembered. Storage may throw or be empty. */
export function readLastWallpaper(): string | null {
  try { return globalThis.localStorage?.getItem(LAST_BG_KEY) || null; } catch { return null; }
}
export function rememberWallpaper(id: string): void {
  try { globalThis.localStorage?.setItem(LAST_BG_KEY, id); } catch { /* ignore */ }
}

/** The last shown bundled wallpaper, else a deterministic generic one (any bundled one when none is generic); null only without any bundled image. */
export function floorWallpaper(list: readonly Wallpaper[], lastId: string | null, seed = 'mango'): Wallpaper | null {
  const last = lastId ? list.find((w) => w.id === lastId) : undefined;
  if (last) return last;
  const generic = list.filter((w) => w.generic);
  const from = generic.length ? generic : list;
  return from.length ? from[hashId(seed) % from.length] : null;
}

/** Stable identity of what a resolved image / panorama draws (the first URL), used to mark a slide bad for the session. */
export const artUrl = (r: { kind: string; url?: string; set?: { faces: string[] } }): string | null =>
  r.kind === 'image' ? r.url ?? null : r.kind === 'panorama' ? r.set?.faces[0] ?? null : null;

/** The floor first, then every other bundled wallpaper (generic ones first): the order in which the engine tries them when an image fails. */
export function floorCandidates(list: readonly Wallpaper[], lastId: string | null, seed = 'mango'): Wallpaper[] {
  const first = floorWallpaper(list, lastId, seed);
  if (!first) return [];
  const rest = [...list.filter((w) => w.generic), ...list.filter((w) => !w.generic)].filter((w) => w.id !== first.id);
  return [first, ...rest];
}
