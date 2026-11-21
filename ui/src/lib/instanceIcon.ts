// Pure rules for instance icons and backgrounds (docs/ipc.md Parts 6 and 7). No Svelte, no Tauri: unit tested.
import { hashId } from './fallbackImage';
import { DEFAULT_SCRIM, MANGO_CLIENT_ID, type Wallpaper } from './wallpapers';
import { resolveAuto, type PoolItem } from './bgCycle';
import type { BlockIcon, Loader, BlockIconsStatus, InstanceIcon, PanoramaSet, ProfileBackground, ProfileView } from './types';

/** Reserved `iconBlock` value (mirrors `BRAND_ICON_ID` in Rust): the real MangoClient icon as an explicit choice. */
export const BRAND_ICON_ID = 'mangoclient';

export type IconInput = Pick<ProfileView, 'coverUrl' | 'icon' | 'autoBlock'>;

/** What an instance shows. The MangoClient icon is the only static fallback, never a letter. */
export type ResolvedIcon =
  | { kind: 'image'; src: string }
  | { kind: 'block'; block: BlockIcon; auto: boolean }
  | { kind: 'brand' };

export function findBlock(status: BlockIconsStatus | null | undefined, id: string | null | undefined): BlockIcon | null {
  if (!status || status.state !== 'ready' || !id) return null;
  return status.blocks.find((b) => b.id === id) ?? null;
}

/**
 * Order: (1) the image icon (`coverUrl`, a custom or modpack image), (2) the chosen block, (3) `autoBlock` once the gallery is
 * ready, (4) the MangoClient icon. A chosen block that is not in the gallery (yet) falls through to the auto block.
 */
export function resolveInstanceIcon(p: IconInput | null | undefined, status: BlockIconsStatus | null | undefined): ResolvedIcon {
  if (!p) return { kind: 'brand' };
  const icon: InstanceIcon = p.icon ?? null;
  // a cover without an icon record is a legacy cover: still an image
  if (p.coverUrl && icon?.kind !== 'block') return { kind: 'image', src: p.coverUrl };
  if (icon?.kind === 'block' && icon.block === BRAND_ICON_ID) return { kind: 'brand' };
  if (icon?.kind === 'block') {
    const b = findBlock(status, icon.block);
    if (b) return { kind: 'block', block: b, auto: false };
  }
  const auto = findBlock(status, p.autoBlock);
  if (auto) return { kind: 'block', block: auto, auto: true };
  return { kind: 'brand' };
}

export type PickerState = { state: 'missing' } | { state: 'preparing' } | { state: 'ready'; blocks: BlockIcon[] } | { state: 'error'; key: string };

/** Message key (`blocks.err.*`) for a `blocks:status` errorCode. */
export function blockErrorKey(code: string | undefined): string {
  switch (code) {
    case 'blocks.incomplete': return 'blocks.err.incomplete';
    case 'blocks.jar_unreadable': return 'blocks.err.jar_unreadable';
    case 'blocks.bad_manifest': return 'blocks.err.bad_manifest';
    default: return code && /^(net|http|offline)/.test(code) ? 'blocks.err.net' : 'blocks.err.generic';
  }
}

/** Map the backend status onto what the picker draws. `null` (not loaded yet) behaves like `missing`. */
export function pickerState(status: BlockIconsStatus | null | undefined): PickerState {
  if (!status || status.state === 'missing') return { state: 'missing' };
  if (status.state === 'preparing') return { state: 'preparing' };
  if (status.state === 'error') return { state: 'error', key: blockErrorKey(status.errorCode) };
  return { state: 'ready', blocks: status.blocks };
}

/** Which block id a draft/profile selection highlights: the explicit block, `auto` for none. */
export const selectedBlockId = (icon: InstanceIcon): string | 'auto' => (icon?.kind === 'block' ? icon.block : 'auto');

/** Deterministic block for a seed (used where no backend `autoBlock` exists yet, i.e. the new-instance dialog). */
export function blockForSeed(seed: string, blocks: readonly BlockIcon[]): BlockIcon | null {
  return blocks.length ? blocks[hashId(seed) % blocks.length] : null;
}

export type ResolvedBackground =
  | { kind: 'image'; url: string; scrim: number; source: 'wallpaper' | 'custom' }
  | { kind: 'panorama'; set: PanoramaSet }
  | { kind: 'watermark' }   // nothing to show yet: token surface + MangoClient icon
  | { kind: 'plain' };      // the user chose none

export interface BackgroundContext {
  mcVersion: string; profileId: string;
  panorama: PanoramaSet | null | undefined;
  wallpapers: readonly Wallpaper[];
  /** Version wallpaper + generic wallpapers + panorama sets (bgCycle.buildPool); empty = the instance's own MangoClient surface. */
  pool?: readonly PoolItem[];
}

/** A pool member as something a hero can draw. */
export function resolvePoolItem(i: PoolItem): ResolvedBackground {
  return i.kind === 'wallpaper' ? { kind: 'image', url: i.url, scrim: i.scrim, source: 'wallpaper' } : i.kind === 'panorama' ? { kind: 'panorama', set: i.set } : { kind: 'watermark' };
}

/**
 * image -> that image; none -> plain surface; panorama -> the Minecraft panorama (watermark until it is extracted);
 * wallpaper -> the bundled wallpaper with that id (auto when it no longer exists); auto -> version wallpaper, else the instance's own
 * panorama, else a deterministic pool member (generic wallpapers + panorama sets).
 */
export function resolveBackground(bg: ProfileBackground | null | undefined, c: BackgroundContext): ResolvedBackground {
  const b = bg ?? { kind: 'auto' as const };
  if (b.kind === 'image') return { kind: 'image', url: b.url, scrim: DEFAULT_SCRIM, source: 'custom' };
  if (b.kind === 'none') return { kind: 'plain' };
  if (b.kind === 'panorama') return c.panorama && c.panorama.faces.length === 6 ? { kind: 'panorama', set: c.panorama } : { kind: 'watermark' };
  if (b.kind === 'wallpaper') {
    if (b.id === MANGO_CLIENT_ID) return { kind: 'watermark' };
    const w = c.wallpapers.find((x) => x.id === b.id);
    if (w) return { kind: 'image', url: w.url, scrim: w.scrim, source: 'wallpaper' };
  }
  // auto: (a) the wallpaper whose version rule matches, (b) the instance's own panorama, (c) a deterministic pick from the pool
  return resolvePoolItem(resolveAuto({ mcVersion: c.mcVersion, profileId: c.profileId, wallpapers: c.wallpapers, ownPanorama: c.panorama, pool: c.pool ?? [] }).item);
}

/** Overlay strength and blur (px) a background is drawn with: the instance's dim/blur override, else the resolved defaults. */
export const BLUR_MAX = 24;
export const PANORAMA_BLUR = 6;
export function backgroundStyle(bg: ProfileBackground | null | undefined, res: ResolvedBackground): { dim: number; blur: number } {
  const dim = bg && 'dim' in bg && typeof bg.dim === 'number' ? bg.dim : res.kind === 'image' ? res.scrim : DEFAULT_SCRIM;
  const blur = bg && 'blur' in bg && typeof bg.blur === 'number' ? bg.blur : res.kind === 'panorama' ? PANORAMA_BLUR : 0;
  return { dim, blur };
}

/** Start angle (degrees, 0..359) of the rotating panorama: derived from the id so instances differ, stable across runs. */
export const panoramaAngle = (profileId: string): number => hashId(profileId) % 360;

/** Seconds per revolution. */
export const PANORAMA_SECONDS = 120;
/** Negative animation delay that starts the shared 0..360 keyframe at `angle`. */
export const panoramaDelay = (angle: number): number => -(angle / 360) * PANORAMA_SECONDS;

/** Instance colour, else the loader's token colour. The tile surface of a block icon is derived from it. */
export const LOADER_TINT: Record<Loader, string> = {
  vanilla: 'var(--leaf)', fabric: 'var(--sky)', quilt: 'var(--mango)', forge: 'var(--tomato)', neoforge: 'var(--mango)',
};
export const iconTint = (p: Pick<ProfileView, 'color' | 'loader'> | null | undefined): string | null => (p ? p.color ?? LOADER_TINT[p.loader] : null);

/** What the icon picker edits: auto, a block, a new image (path from the OS dialog), or `keep` (the image the instance already has). */
export type IconChoice =
  | { kind: 'auto' } | { kind: 'keep' } | { kind: 'block'; block: string }
  | { kind: 'image'; path: string; name: string; preview: string | null };
export const choiceFromIcon = (icon: InstanceIcon): IconChoice => (icon?.kind === 'block' ? { kind: 'block', block: icon.block } : icon?.kind === 'image' ? { kind: 'keep' } : { kind: 'auto' });
export function sameChoice(a: IconChoice, b: IconChoice): boolean {
  if (a.kind !== b.kind) return false;
  if (a.kind === 'block' && b.kind === 'block') return a.block === b.block;
  if (a.kind === 'image' && b.kind === 'image') return a.path === b.path;
  return true;
}

/** What the background picker edits. `keep` = the image the instance already has. */
export type BackgroundChoice =
  | { kind: 'auto' } | { kind: 'none' } | { kind: 'keep' } | { kind: 'panorama' }
  | { kind: 'screenshot'; name: string; url: string }
  | { kind: 'image'; path: string; name: string; preview: string | null }
  | { kind: 'wallpaper'; id: string; label: string; url: string };
export function choiceFromBackground(b: ProfileBackground, list: readonly Wallpaper[] = []): BackgroundChoice {
  switch (b.kind) {
    case 'image': return { kind: 'keep' };
    case 'none': return { kind: 'none' };
    case 'panorama': return { kind: 'panorama' };
    case 'wallpaper': {
      if (b.id === MANGO_CLIENT_ID) return { kind: 'wallpaper', id: b.id, label: 'Mango Client', url: '' };
      const w = list.find((x) => x.id === b.id);
      return w ? { kind: 'wallpaper', id: w.id, label: w.label, url: w.url } : { kind: 'auto' };
    }
    default: return { kind: 'auto' };
  }
}
export function sameBackground(a: BackgroundChoice, b: BackgroundChoice): boolean {
  if (a.kind !== b.kind) return false;
  if (a.kind === 'screenshot' && b.kind === 'screenshot') return a.name === b.name;
  if (a.kind === 'image' && b.kind === 'image') return a.path === b.path;
  if (a.kind === 'wallpaper' && b.kind === 'wallpaper') return a.id === b.id;
  return true;
}
/** The background the mini preview draws for a draft; `current` is what the instance has now. */
export function previewBackground(c: BackgroundChoice, current: ProfileBackground): ProfileBackground {
  switch (c.kind) {
    case 'auto': return { kind: 'auto' };
    case 'panorama': return { kind: 'panorama' };
    case 'none': return { kind: 'none' };
    case 'keep': return current;
    case 'wallpaper': return { kind: 'wallpaper', id: c.id };
    case 'screenshot': return { kind: 'image', url: c.url };
    case 'image': return c.preview ? { kind: 'image', url: c.preview } : current;
  }
}
