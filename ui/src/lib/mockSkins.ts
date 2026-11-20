// Browser mock of docs/ipc.md Part 13 "Skins". Textures are flat colour-coded UV test maps drawn on a canvas at runtime (every body part a
// colour, a marker on the face) so orientation, slim vs classic arms and the overlay layer can be checked; no artwork, nothing shipped.
// Without a canvas (unit tests) the images are 1x1 PNGs. `?mockSkinError=<code>` makes uploads / username copies / cape changes fail.
import type { Api } from './api';
import { faceUv, playerParts, type Face } from './skin3d';
import { hashId } from './fallbackImage';
import type { ApiEvents, CapeInfo, LibrarySkin, PublicAccount, SkinInfo, SkinVariantName } from './types';

const ONE_PX = 'data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==';
const fail = (code: string, message = code) => ({ code, message, needsRelogin: false });
const canvasOk = (): boolean => typeof document !== 'undefined' && typeof document.createElement === 'function';

interface Palette { skin: string; hair: string; shirt: string; pants: string; shoes: string }
const PALETTES: Palette[] = [
  { skin: '#c68e6b', hair: '#3b2a1e', shirt: '#2f9ea8', pants: '#3d4f9e', shoes: '#555a63' },
  { skin: '#e8b995', hair: '#c9a227', shirt: '#c0432f', pants: '#2c2f38', shoes: '#25262b' },
  { skin: '#8d5a3b', hair: '#161616', shirt: '#7a3fa0', pants: '#4a4a52', shoes: '#2d5b34' },
  { skin: '#f0cfae', hair: '#8c4a1c', shirt: '#5a9e3a', pants: '#6b5a3a', shoes: '#3a2f22' },
];
export const paletteFor = (seed: string): Palette => PALETTES[hashId(seed) % PALETTES.length];

function colorOf(name: string, face: Face, p: Palette): string {
  const base = /^(head|hat)$/.test(name) ? (face === 'front' ? p.skin : p.hair)
    : /Arm|Sleeve/.test(name) ? (face === 'top' || face === 'bottom' ? p.skin : p.shirt)
    : /^(body|jacket)$/.test(name) ? p.shirt
    : face === 'bottom' ? p.shoes : p.pants;
  return base;
}

function skinCanvas(p: Palette, slim: boolean): HTMLCanvasElement {
  const c = document.createElement('canvas'); c.width = 64; c.height = 64;
  const ctx = c.getContext('2d') as CanvasRenderingContext2D;
  for (const part of playerParts(slim)) {
    const [w, h, d] = part.size;
    for (const f of ['front', 'back', 'right', 'left', 'top', 'bottom'] as Face[]) {
      const [x, y, fw, fh] = faceUv(f, w, h, d, part.uv[0], part.uv[1]);
      ctx.globalAlpha = part.overlay ? 0.45 : 1;
      ctx.fillStyle = colorOf(part.name, f, p);
      ctx.fillRect(x, y, fw, fh);
      // a darker outline pixel row on the lower edge so the unwrap reads
      ctx.globalAlpha = part.overlay ? 0.25 : 0.35; ctx.fillStyle = '#000';
      ctx.fillRect(x, y + fh - 1, fw, 1);
    }
  }
  ctx.globalAlpha = 1;
  // face marker on the head front (8,8): two eyes and a mouth
  ctx.fillStyle = '#fff'; ctx.fillRect(9, 11, 2, 1); ctx.fillRect(13, 11, 2, 1);
  ctx.fillStyle = '#2a1a14'; ctx.fillRect(10, 11, 1, 1); ctx.fillRect(14, 11, 1, 1); ctx.fillRect(10, 14, 4, 1);
  // the back of the body carries a stripe so the back is recognisable
  ctx.fillStyle = '#fff'; ctx.globalAlpha = 0.5; ctx.fillRect(32, 24, 8, 1);
  ctx.globalAlpha = 1;
  return c;
}
const CAPE_COLORS = ['#b83a3a', '#3a6fb8', '#d6a63a', '#4aa36a'];
function capeCanvas(color: string): HTMLCanvasElement {
  const c = document.createElement('canvas'); c.width = 64; c.height = 32;
  const ctx = c.getContext('2d') as CanvasRenderingContext2D;
  ctx.fillStyle = color; ctx.fillRect(1, 1, 10, 16);                       // outer face
  ctx.fillStyle = '#fff'; ctx.globalAlpha = 0.28; ctx.fillRect(1, 1, 10, 3); ctx.fillRect(4, 6, 4, 6); ctx.globalAlpha = 1;
  ctx.fillStyle = '#22252b'; ctx.fillRect(12, 1, 10, 16);                   // inner face
  ctx.fillStyle = color; ctx.fillRect(0, 1, 1, 16); ctx.fillRect(11, 1, 1, 16); ctx.fillRect(1, 0, 10, 1); ctx.fillRect(11, 0, 10, 1);
  return c;
}
const toUrl = (c: HTMLCanvasElement): string => c.toDataURL('image/png');
export const mockSkinUrl = (seed: string, slim: boolean): string => (canvasOk() ? toUrl(skinCanvas(paletteFor(seed), slim)) : ONE_PX);
export const mockCapeUrl = (i: number): string => (canvasOk() ? toUrl(capeCanvas(CAPE_COLORS[i % CAPE_COLORS.length])) : ONE_PX);

/** 8x8 head (face + hat) of a skin image URL as a data URL; null when no canvas / image cannot be read. */
export async function headOf(url: string): Promise<string | null> {
  if (!canvasOk()) return null;
  return new Promise((resolve) => {
    const img = new Image();
    img.onload = () => {
      try {
        const c = document.createElement('canvas'); c.width = 64; c.height = 64;
        const ctx = c.getContext('2d') as CanvasRenderingContext2D;
        ctx.imageSmoothingEnabled = false;
        ctx.drawImage(img, 8, 8, 8, 8, 0, 0, 64, 64);
        ctx.drawImage(img, 40, 8, 8, 8, 0, 0, 64, 64);
        resolve(c.toDataURL('image/png'));
      } catch { resolve(null); }
    };
    img.onerror = () => resolve(null);
    img.src = url;
  });
}

export interface SkinsMockCtx {
  scale: number;
  emit: <K extends keyof ApiEvents>(name: K, payload: ApiEvents[K]) => void;
  accounts: () => PublicAccount[];
  /** Replaces the account's head (a fresh `?v=` in the real service). */
  setHead: (uuid: string, url: string) => void;
  /** `?mockSkinError=<code>` */
  error?: string;
  /** `?mockCapes=0`: the account owns no capes. */
  noCapes?: boolean;
}

export type SkinsApi = Pick<Api, 'skinGet' | 'skinUpload' | 'skinUseUsername' | 'skinSetCape' | 'skinLibraryList' | 'skinLibraryAdd' | 'skinLibrarySaveCurrent' | 'skinLibraryRemove' | 'skinLibraryRename' | 'skinLibraryApply'>;

const NAME = /^[A-Za-z0-9_]{3,16}$/;
const CAPE_NAMES = ['Migrator', 'Vanilla', 'Mojang Office', 'Cherry Blossom'];

export function createSkinsMock(ctx: SkinsMockCtx): { api: SkinsApi; init: () => void } {
  interface State { skinUrl: string; slim: boolean; capeId: string | null; v: number }
  const states = new Map<string, State>();
  const capeUrls = CAPE_NAMES.map((_, i) => mockCapeUrl(i));
  let seq = 0;
  let library: LibrarySkin[] = [
    { id: 'lib-1', name: 'Forest Ranger', variant: 'classic', url: mockSkinUrl('forest-ranger', false), addedAt: Date.now() - 9 * 86400000 },
    { id: 'lib-2', name: 'Night Shift', variant: 'slim', url: mockSkinUrl('night-shift', true), addedAt: Date.now() - 3 * 86400000 },
    { id: 'lib-3', name: 'Redstone Tinkerer', variant: 'classic', url: mockSkinUrl('redstone', false), addedAt: Date.now() - 86400000 },
  ];
  const delay = () => new Promise((r) => setTimeout(r, 120 * ctx.scale));
  const account = (uuid: string): PublicAccount => {
    const a = ctx.accounts().find((x) => x.uuid === uuid);
    if (!a) throw fail('auth.invalid_account', 'no such account');
    return a;
  };
  const state = (a: PublicAccount): State => {
    let s = states.get(a.uuid);
    if (!s) {
      const slim = a.type === 'microsoft' && hashId(a.uuid) % 2 === 1;
      s = { skinUrl: a.type === 'offline' ? mockSkinUrl('steve-default', false) : mockSkinUrl(a.name, slim), slim: a.type === 'offline' ? false : slim, capeId: a.hasCape && !ctx.noCapes ? 'cape-0' : null, v: 1 };
      states.set(a.uuid, s);
    }
    return s;
  };
  const info = (a: PublicAccount): SkinInfo => {
    const s = state(a);
    const online = a.type === 'microsoft';
    const capes: CapeInfo[] = online && !ctx.noCapes ? [0, 1, 2].map((i) => ({ id: `cape-${i}`, alias: CAPE_NAMES[i], url: `${capeUrls[i]}`, active: s.capeId === `cape-${i}` })) : [];
    const active = capes.find((c) => c.active);
    return { skinUrl: s.skinUrl, capeUrl: active?.url ?? null, slim: s.slim, source: online ? 'account' : 'default', capes, canEdit: online, stale: false };
  };
  const editable = (uuid: string): PublicAccount => {
    const a = account(uuid);
    if (a.type !== 'microsoft') throw fail('skin.offline_account', 'skins of offline accounts cannot be changed');
    if (a.needsRelogin) throw { code: 'auth.needs_relogin', message: 'sign in again', needsRelogin: true };
    if (ctx.error) throw fail(ctx.error);
    return a;
  };
  const changed = async (a: PublicAccount): Promise<SkinInfo> => {
    const s = state(a);
    s.v += 1;
    const head = await headOf(s.skinUrl);
    if (head) ctx.setHead(a.uuid, head);
    ctx.emit('skin:updated', { uuid: a.uuid });
    return info(a);
  };
  const pathOk = (p: string): void => {
    if (!p) throw fail('path.not_granted');
    if (!/^(blob:|data:image\/png)/.test(p) && !/\.png$/i.test(p)) throw fail('skin.invalid_png', 'not a PNG');
  };

  const api: SkinsApi = {
    async skinGet({ uuid }) { await delay(); return info(account(uuid)); },
    async skinUpload({ uuid, filePath, variant }) {
      const a = editable(uuid); await delay(); pathOk(filePath);
      Object.assign(state(a), { skinUrl: filePath, slim: variant === 'slim' });
      return changed(a);
    },
    async skinUseUsername({ uuid, name, variant }) {
      const a = editable(uuid); await delay();
      if (!NAME.test(name)) throw fail('auth.invalid_username');
      if (/^(nobody|unknown)/i.test(name)) throw fail('skin.not_found', 'no such player');
      const slim = variant ? variant === 'slim' : hashId(name) % 2 === 1;
      Object.assign(state(a), { skinUrl: mockSkinUrl(name, slim), slim });
      return changed(a);
    },
    async skinSetCape({ uuid, capeId }) {
      const a = editable(uuid); await delay();
      if (capeId !== null && !info(a).capes.some((c) => c.id === capeId)) throw fail('validation', 'unknown cape');
      state(a).capeId = capeId;
      return changed(a);
    },
    async skinLibraryList() { return structuredClone(library).sort((x, y) => y.addedAt - x.addedAt); },
    async skinLibraryAdd({ filePath, name, variant }) {
      pathOk(filePath);
      if (library.length >= 200) throw fail('skin.library_full');
      const s: LibrarySkin = { id: `lib-new-${++seq}`, name: (name ?? 'Skin').slice(0, 48), variant: variant as SkinVariantName, url: filePath, addedAt: Date.now() };
      library = [...library, s];
      return structuredClone(s);
    },
    async skinLibrarySaveCurrent({ uuid, name }) {
      const a = account(uuid); await delay();
      if (library.length >= 200) throw fail('skin.library_full');
      const s = state(a);
      const e: LibrarySkin = { id: `lib-new-${++seq}`, name: (name ?? a.name).slice(0, 48), variant: s.slim ? 'slim' : 'classic', url: s.skinUrl, addedAt: Date.now() };
      library = [...library, e];
      return structuredClone(e);
    },
    async skinLibraryRemove({ id }) {
      if (!library.some((x) => x.id === id)) throw fail('skin.not_found');
      library = library.filter((x) => x.id !== id);
      return structuredClone(library);
    },
    async skinLibraryRename({ id, name }) {
      const n = name.trim();
      if (!n || n.length > 48 || /[\u0000-\u001f\u007f]/.test(n)) throw fail('skin.invalid_name');
      const e = library.find((x) => x.id === id);
      if (!e) throw fail('skin.not_found');
      e.name = n;
      return structuredClone(e);
    },
    async skinLibraryApply({ uuid, id }) {
      const a = editable(uuid); await delay();
      const e = library.find((x) => x.id === id);
      if (!e) throw fail('skin.not_found');
      Object.assign(state(a), { skinUrl: e.url, slim: e.variant === 'slim' });
      return changed(a);
    },
  };
  /** Gives the seeded accounts a head matching their skin. */
  const init = (): void => {
    for (const a of ctx.accounts()) void headOf(state(a).skinUrl).then((h) => { if (h) ctx.setHead(a.uuid, h); });
  };
  return { api, init };
}
