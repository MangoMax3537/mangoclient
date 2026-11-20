// Part 4 mock catalogue: per-project compatibility, dependencies and conflicts.
// Pure data and pure functions (no timers), shared by mockContent.ts and the tests.
import { DEPS, MODPACKS, rowOf, TABLES, type Row } from './mockData';
import type { ContentType, ProjectType } from './types';

export const hash = (s: string): number => { let h = 2166136261; for (let i = 0; i < s.length; i++) h = Math.imul(h ^ s.charCodeAt(i), 16777619); return h >>> 0; };

export interface ProjMeta {
  loaders: string[];
  gameVersions: string[];
  required: string[];
  optional: string[];
  /** Installed projects this one cannot run with. */
  conflicts: string[];
}
const ALL_GV = ['1.21.4', '1.21.3', '1.21.1', '1.21', '1.20.6', '1.20.4', '1.20.1'];
const FEATURED: Record<string, Partial<ProjMeta>> = {
  'sodium-extra': { loaders: ['fabric', 'quilt', 'neoforge'], gameVersions: ['1.21.4', '1.21.1'] },
  'reeses-sodium-options': { loaders: ['fabric', 'quilt', 'neoforge'], gameVersions: ['1.21.4', '1.21.1'] },
  sodium: { loaders: ['fabric', 'quilt', 'neoforge'], gameVersions: ['1.21.4', '1.21.3', '1.21.1'], optional: ['sodium-extra', 'reeses-sodium-options'] },
  iris: { loaders: ['fabric', 'quilt', 'neoforge'], gameVersions: ['1.21.4', '1.21.1', '1.20.6'], optional: ['complementary-reimagined'] },
  'fabric-api': { loaders: ['fabric', 'quilt'], gameVersions: ALL_GV },
  lithium: { loaders: ['fabric', 'quilt', 'neoforge', 'forge'], gameVersions: ['1.21.4', '1.21.1', '1.20.1'] },
  'create-fabric': { loaders: ['fabric'], gameVersions: ['1.21.1'], optional: ['modmenu'] },
  'farmers-delight': { loaders: ['neoforge', 'forge'], gameVersions: ['1.21.1', '1.20.1'] },
  'cull-leaves': { loaders: ['fabric'], gameVersions: ['1.21.4'], conflicts: ['moreculling'] },
  immediatelyfast: { loaders: ['fabric', 'neoforge', 'forge'], gameVersions: ['1.21.4', '1.21.1', '1.20.1'] },
  'xaeros-minimap': { loaders: ['fabric', 'quilt', 'neoforge', 'forge'], gameVersions: ALL_GV },
  zoomify: { loaders: ['fabric', 'quilt'], gameVersions: ['1.21.4', '1.21.1'], optional: ['modmenu'] },
  rei: { loaders: ['fabric', 'neoforge', 'forge'], gameVersions: ['1.21.4', '1.21.1', '1.20.1'] },
  modmenu: { loaders: ['fabric', 'quilt'], gameVersions: ['1.21.4', '1.21.1', '1.20.6'] },
};
/** New required dependencies an update brings (so the update preview has something to show). */
export const UPDATE_NEW_DEPS: Record<string, string[]> = { modmenu: ['terrablender'] };

export function typeOfSlug(slug: string): ProjectType {
  const r = rowOf(slug);
  if (r) return r.type;
  return 'mod';
}
export function rowOfSlug(slug: string): Row | null {
  return rowOf(slug)?.row ?? null;
}
export const catalogFor = (type: ProjectType): Row[] => TABLES[type] ?? [];

export function metaOf(slug: string): ProjMeta {
  const type = typeOfSlug(slug);
  const h = hash(slug);
  const f = FEATURED[slug] ?? {};
  const gv = ALL_GV.filter((_, i) => (i === 0 ? h % 10 < 8 : (h >> i) % 3 !== 0));
  const loaders = type === 'shader' ? ['iris', 'optifine'] : type === 'resourcepack' ? ['minecraft']
    : ['fabric', ...(h % 3 ? ['quilt'] : []), ...(h % 4 === 0 ? ['forge', 'neoforge'] : [])];
  return {
    loaders: f.loaders ?? loaders,
    gameVersions: f.gameVersions ?? (gv.length ? gv : ['1.21.1']),
    required: f.required ?? DEPS[slug] ?? [],
    optional: f.optional ?? [],
    conflicts: f.conflicts ?? [],
  };
}

/** Loaders a profile loader accepts builds of (Quilt also runs Fabric builds, NeoForge also Forge builds). */
export const accepts = (loader: string): string[] => (loader === 'quilt' ? ['quilt', 'fabric'] : loader === 'neoforge' ? ['neoforge', 'forge'] : [loader]);

export type Compat = { ok: true } | { ok: false; reason: 'loader' | 'mc_version' | 'no_compatible_version' };
/** Mirrors the rules in docs/ipc.md (install_targets reasons) for mods; packs and shaders ignore the loader. */
export function compat(slug: string, type: ContentType, mcVersion: string, loader: string): Compat {
  const m = metaOf(slug);
  const mc = m.gameVersions.includes(mcVersion);
  if (type !== 'mod') return mc ? { ok: true } : { ok: false, reason: 'mc_version' };
  if (loader === 'vanilla') return { ok: false, reason: 'loader' };
  const ld = accepts(loader).some((l) => m.loaders.includes(l));
  if (mc && ld) return { ok: true };
  if (mc) return { ok: false, reason: 'loader' };
  if (ld) return { ok: false, reason: 'mc_version' };
  return { ok: false, reason: 'no_compatible_version' };
}

export interface PackMeta { mcVersion: string; loader: 'fabric' | 'forge' | 'neoforge' | 'quilt'; loaderVersion: string; files: number; bytes: number }
const PACKS: Record<string, PackMeta> = {
  'fabulously-optimized': { mcVersion: '1.21.4', loader: 'fabric', loaderVersion: '0.16.10', files: 61, bytes: 38_400_000 },
  'better-mc-fabric': { mcVersion: '1.21.1', loader: 'fabric', loaderVersion: '0.16.9', files: 212, bytes: 190_300_000 },
  'prominence-ii': { mcVersion: '1.20.1', loader: 'forge', loaderVersion: '47.3.0', files: 268, bytes: 412_900_000 },
};
export function packMeta(slug: string): PackMeta {
  const h = hash(slug);
  return PACKS[slug] ?? { mcVersion: '1.21.1', loader: 'fabric', loaderVersion: '0.16.9', files: 20 + (h % 120), bytes: 8_000_000 + (h % 90_000_000) };
}
export const MODPACK_SLUGS = new Set(MODPACKS.map((r) => r[0]));

/** Stable fake file size per project. */
export const sizeOf = (slug: string): number => 90_000 + (hash(`size:${slug}`) % 4_800_000);
export const websiteOf = (slug: string, type: ProjectType): string => `https://modrinth.com/${type}/${slug}`;
