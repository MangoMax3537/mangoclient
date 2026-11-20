// Curated wallpapers, as data: ./assets/backgrounds/backgrounds.json lists them, the image files sit next to it (webp, jpg, jpeg, png).
// Entries whose image is missing are skipped; an empty or missing folder gives an empty list. Nothing here is hard-coded art.
import { hashId } from './fallbackImage';

export interface WallpaperEntry { file: string; label: string; versions?: string[]; generic?: boolean; scrim?: number }
export interface Wallpaper { id: string; file: string; label: string; url: string; path: string; versions: string[]; generic: boolean; scrim: number }

export const DEFAULT_SCRIM = 0.6;
export const clampScrim = (n: unknown): number => (typeof n === 'number' && isFinite(n) ? Math.min(0.9, Math.max(0.3, n)) : DEFAULT_SCRIM);

const EXT = /\.(webp|jpe?g|png)$/i;
const stem = (f: string): string => (f.split('/').pop() ?? f).replace(EXT, '').toLowerCase();

/** `1.21.4` -> [1,21,4]; anything non-numeric (snapshots such as 24w40a) -> null. */
export function parseVersion(v: string): number[] | null {
  if (!/^\d+(\.\d+)*$/.test(v)) return null;
  return v.split('.').map(Number);
}
export function compareVersions(a: number[], b: number[]): number {
  for (let i = 0; i < Math.max(a.length, b.length); i++) {
    const d = (a[i] ?? 0) - (b[i] ?? 0);
    if (d) return d < 0 ? -1 : 1;
  }
  return 0;
}
/** Exact id, prefix wildcard (`1.20.*` = the 1.20 line, `26.3*` = starts with) or inclusive range (`1.21.6-1.21.8`, numeric-aware). */
export function versionMatches(pattern: string, mc: string): boolean {
  const p = pattern.trim();
  if (!p) return false;
  if (p.endsWith('*')) {
    const base = p.slice(0, -1);
    if (base.endsWith('.')) { const b = base.slice(0, -1); return mc === b || mc.startsWith(`${b}.`); }
    return mc.startsWith(base);
  }
  const m = mc && parseVersion(mc);
  if (p.includes('-')) {
    const [lo, hi] = p.split('-').map((x) => parseVersion(x.trim()));
    return !!(m && lo && hi && compareVersions(m, lo) >= 0 && compareVersions(m, hi) <= 0);
  }
  const e = parseVersion(p);
  return !!(m && e ? compareVersions(m, e) === 0 : p === mc);
}

/** Pure: manifest + (path -> url) image map to the usable wallpapers. Skips entries without an image and malformed ones. */
export function buildWallpapers(manifest: unknown, images: Record<string, string> | null | undefined): Wallpaper[] {
  const byStem = new Map<string, { file: string; url: string }>();
  for (const [path, url] of Object.entries(images ?? {})) {
    const file = path.split('/').pop() ?? path;
    if (EXT.test(file) && url && !byStem.has(stem(file))) byStem.set(stem(file), { file, url });
  }
  const raw = (manifest as { wallpapers?: unknown } | null)?.wallpapers;
  const out: Wallpaper[] = [];
  const used = new Set<string>();
  for (const e of Array.isArray(raw) ? (raw as Partial<WallpaperEntry>[]) : []) {
    if (!e || typeof e.file !== 'string') continue;
    const img = byStem.get(stem(e.file));
    if (!img) continue;
    used.add(stem(e.file));
    out.push({
      id: stem(e.file), file: img.file, url: img.url, path: `assets/backgrounds/${img.file}`,
      label: typeof e.label === 'string' && e.label ? e.label : stem(e.file),
      versions: Array.isArray(e.versions) ? e.versions.filter((v) => typeof v === 'string') : [],
      // an entry with version rules is never generic
      generic: e.generic === true && !(Array.isArray(e.versions) && e.versions.length > 0), scrim: clampScrim(e.scrim),
    });
  }
  // images dropped in without a manifest entry are still offered in the picker (never picked automatically)
  for (const [s, img] of byStem) {
    if (used.has(s)) continue;
    out.push({ id: s, file: img.file, url: img.url, path: `assets/backgrounds/${img.file}`, label: s.replace(/[-_]+/g, ' ').replace(/\b\w/g, (c) => c.toUpperCase()), versions: [], generic: false, scrim: DEFAULT_SCRIM });
  }
  return out;
}

/** Every wallpaper once (the first entry per image), for the picker. */
export const uniqueWallpapers = (list: readonly Wallpaper[]): Wallpaper[] => {
  const seen = new Set<string>();
  return list.filter((w) => (seen.has(w.id) ? false : (seen.add(w.id), true)));
};

export type AutoRule = 'version' | 'generic';
/**
 * Auto background: (1) entries whose versions match (several: deterministic pick by the hash of the profile id),
 * (2) else the generic pool, same pick, (3) else null.
 */
export function autoWallpaper(list: readonly Wallpaper[], mcVersion: string, profileId: string): { wallpaper: Wallpaper; rule: AutoRule } | null {
  const pick = (l: Wallpaper[]) => l[hashId(profileId) % l.length];
  const v = list.filter((w) => !w.generic && w.versions.some((p) => versionMatches(p, mcVersion)));
  if (v.length) return { wallpaper: pick(v), rule: 'version' };
  const pool = list.filter((w) => w.generic);
  return pool.length ? { wallpaper: pick(pool), rule: 'generic' } : null;
}

const images = import.meta.glob('@backgrounds/*.{webp,jpg,jpeg,png,WEBP,JPG,JPEG,PNG}', { eager: true, query: '?url', import: 'default' }) as Record<string, string>;
const manifests = import.meta.glob('@backgrounds/backgrounds.json', { eager: true, import: 'default' }) as Record<string, unknown>;

export const wallpapers: Wallpaper[] = buildWallpapers(Object.values(manifests)[0], images);
