// Mock backend for docs/ipc.md Parts 6 + 7 (block gallery, instance icons, panoramas, backgrounds).
// No Mojang art may be committed and no placeholder art generated, so every face is the real MangoClient icon.
import type { Api } from './api';
import { brandIcon, hashId } from './fallbackImage';
import type { ApiEvents, BlockIcon, BlockIconsStatus, PanoramaSet, ProfileBackground, ProfileView } from './types';

/** `?mockBlocks=`: default = missing, then ready after the prepare run; off = prepare does nothing; preparing = never finishes; error = fails. */
export type MockBlocksMode = 'default' | 'off' | 'preparing' | 'error';
/** `?mockPanorama=off`: nothing is ever extracted (watermark surface). */
export type MockPanoramaMode = 'default' | 'off';

export const BLOCK_IDS: readonly (readonly [string, string])[] = [
  ['grass_block', 'Grass Block'], ['dirt', 'Dirt'], ['stone', 'Stone'], ['cobblestone', 'Cobblestone'], ['oak_planks', 'Oak Planks'],
  ['oak_log', 'Oak Log'], ['crafting_table', 'Crafting Table'], ['furnace', 'Furnace'], ['tnt', 'TNT'], ['bricks', 'Bricks'],
  ['bookshelf', 'Bookshelf'], ['diamond_ore', 'Diamond Ore'], ['gold_ore', 'Gold Ore'], ['iron_ore', 'Iron Ore'], ['emerald_ore', 'Emerald Ore'],
  ['redstone_ore', 'Redstone Ore'], ['lapis_block', 'Lapis Block'], ['sandstone', 'Sandstone'], ['netherrack', 'Netherrack'],
  ['obsidian', 'Obsidian'], ['pumpkin', 'Pumpkin'],
];

const fail = (code: string) => ({ code, message: code, needsRelogin: false });
const INSTALLED = ['1.21.4', '1.21.1', '1.8.9', '1.20.6'];

export interface IconsMockCtx {
  scale: number;
  emit: <K extends keyof ApiEvents>(name: K, payload: ApiEvents[K]) => void;
  profiles: () => ProfileView[];
  find: (id: string) => ProfileView;
  blocks?: MockBlocksMode;
  panorama?: MockPanoramaMode;
}

export type IconsApi = Pick<Api,
  'blockIconsStatus' | 'blockIconsPrepare' | 'profileIconSetBlock' | 'profileIconSetImage' | 'profileIconClear' | 'panoramaForProfile'
  | 'profileBackgroundSetScreenshot' | 'profileBackgroundSetImage' | 'profileBackgroundClear'
  | 'profileBackgroundSetStyle' | 'profileBackgroundSetKind' | 'profileIconUseModpackLogo'>;

/** Same shape rule as the Rust side (`ProfileBackground.id`): bundled wallpaper file stem. */
const WALLPAPER_STEM = /^[a-z0-9-]{1,64}$/;

const isUrl = (s: string) => /^(blob:|data:|https?:|\/|\.\/|assets\/)/.test(s) || /\.(png|jpe?g|webp|gif)$/i.test(s) && s.includes('/');

export function createIconsMock(ctx: IconsMockCtx): { api: IconsApi; sync: () => void } {
  const mode = ctx.blocks ?? 'default';
  const faceUrl = brandIcon(256);
  const gallery: BlockIcon[] = BLOCK_IDS.map(([id, label]) => ({ id, label, faces: { top: faceUrl, left: faceUrl, right: faceUrl } }));
  let status: BlockIconsStatus = mode === 'preparing' ? { state: 'preparing', mcVersion: '1.21.4', blocks: [] }
    : mode === 'error' ? { state: 'error', mcVersion: null, blocks: [], errorCode: 'blocks.jar_unreadable' }
    : { state: 'missing', mcVersion: null, blocks: [] };

  /** The deterministic auto rule: hash of the instance id into the gallery, null until the gallery is ready. */
  const sync = () => {
    for (const p of ctx.profiles()) p.autoBlock = status.state === 'ready' ? gallery[hashId(p.id) % gallery.length].id : null;
  };
  const set = (s: BlockIconsStatus) => { status = s; sync(); ctx.emit('blocks:status', structuredClone(s)); };
  const view = (id: string) => structuredClone(ctx.find(id));
  const readCover = (filePath: string): string => {
    if (!filePath) throw fail('image_decode');
    if (!/\.(png|jpe?g|webp|gif)$/i.test(filePath) && !filePath.startsWith('blob:') && !filePath.startsWith('data:')) throw fail('image_unsupported_type');
    return isUrl(filePath) ? filePath : brandIcon(256);
  };
  const asked = new Set<string>();
  /** dim/blur survive kind changes (stored beside the kind, like `backgroundDim` / `backgroundBlur`). */
  const styles = new Map<string, { dim?: number; blur?: number }>();
  /** `covers/<id>-pack.png`: the modpack logo kept next to a custom image. */
  const packLogos = new Map<string, string>();
  const setBg = (id: string, bg: ProfileBackground) => {
    const st = styles.get(id) ?? {};
    ctx.find(id).background = bg.kind === 'none' ? bg : { ...bg, ...(st.dim !== undefined && { dim: st.dim }), ...(st.blur !== undefined && { blur: st.blur }) };
  };
  const stashLogo = (p: ProfileView) => { if (p.iconSource === 'modpack' && p.coverUrl) packLogos.set(p.id, p.coverUrl); };

  const api: IconsApi = {
    async blockIconsStatus() { return structuredClone(status); },
    async blockIconsPrepare() {
      if (mode === 'off' || status.state === 'ready' || status.state === 'preparing' && mode === 'preparing') return structuredClone(status);
      const next: BlockIconsStatus = { state: 'preparing', mcVersion: '1.21.4', blocks: [] };
      setTimeout(() => {
        if (mode === 'error') set({ state: 'error', mcVersion: null, blocks: [], errorCode: 'blocks.jar_unreadable' });
        else set({ state: 'ready', mcVersion: '1.21.4', blocks: structuredClone(gallery) });
      }, 2200 * ctx.scale);
      status = next;
      return structuredClone(next);
    },
    async profileIconSetBlock({ id, block }) {
      const p = ctx.find(id);
      if (status.state !== 'ready' || !gallery.some((b) => b.id === block)) throw fail('block_unknown');
      stashLogo(p); p.icon = { kind: 'block', block }; p.coverUrl = null; p.iconSource = null;
      return view(id);
    },
    async profileIconSetImage({ id, filePath }) {
      const p = ctx.find(id);
      const cover = readCover(filePath);
      stashLogo(p); p.coverUrl = cover; p.icon = { kind: 'image' }; p.iconSource = 'custom';
      return view(id);
    },
    async profileIconClear({ id }) {
      const p = ctx.find(id);
      stashLogo(p); p.icon = null; p.coverUrl = null; p.iconSource = null;
      return view(id);
    },
    async profileIconUseModpackLogo({ id }) {
      const p = ctx.find(id);
      const logo = packLogos.get(id) ?? (p.iconSource === 'modpack' ? p.coverUrl : null);
      if (!logo) throw fail('icon.no_modpack_logo');
      p.coverUrl = logo; p.icon = { kind: 'image' }; p.iconSource = 'modpack';
      return view(id);
    },
    async panoramaForProfile({ id }) {
      const p = ctx.find(id);
      if (ctx.panorama === 'off') return null;
      if (!asked.has(id)) { // extraction runs in the background: null first, `panorama:ready` afterwards
        asked.add(id);
        setTimeout(() => ctx.emit('panorama:ready', { profileId: id }), 1100 * ctx.scale);
        return null;
      }
      const own = INSTALLED.includes(p.mcVersion);
      const set: PanoramaSet = { mcVersion: own ? p.mcVersion : '1.21.4', source: own ? 'own' : 'nearest', faces: Array.from({ length: 6 }, () => brandIcon(512)) };
      return set;
    },
    async profileBackgroundSetScreenshot({ id, name }) {
      if (!/^[^/\\]+\.(png|jpe?g)$/i.test(name)) throw fail('invalid_name');
      setBg(id, { kind: 'image', url: brandIcon(512) });
      return view(id);
    },
    async profileBackgroundSetImage({ id, filePath }) {
      setBg(id, { kind: 'image', url: readCover(filePath) });
      return view(id);
    },
    async profileBackgroundClear({ id, none }) {
      styles.delete(id); // clearing also resets dim and blur
      setBg(id, none ? { kind: 'none' } : { kind: 'auto' });
      return view(id);
    },
    async profileBackgroundSetStyle({ id, dim, blur }) {
      const bg = ctx.find(id).background;
      const ok = (v: number | null | undefined, max: number) => v == null || (Number.isFinite(v) && v >= 0 && v <= max);
      if (!ok(dim, 1) || !ok(blur, 24)) throw fail('background.bad_style');
      const st = { ...styles.get(id) };
      if (dim != null) st.dim = dim;
      if (blur != null) st.blur = blur;
      styles.set(id, st);
      setBg(id, bg);
      return view(id);
    },
    async profileBackgroundSetKind({ id, kind, wallpaper }) {
      ctx.find(id);
      if (kind === 'wallpaper') {
        if (!wallpaper || !WALLPAPER_STEM.test(wallpaper)) throw fail('background.bad_wallpaper');
        setBg(id, { kind: 'wallpaper', id: wallpaper });
      } else if (kind === 'auto' || kind === 'none' || kind === 'panorama') {
        setBg(id, { kind });
      } else throw fail('background.bad_kind');
      return view(id);
    },
  };
  return { api, sync };
}
