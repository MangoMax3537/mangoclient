// Flat 2D rendering of a skin (front / back view): the fallback when WebGL is unavailable and the library previews. Pure rectangle
// maths plus one canvas draw function. Nearest-neighbour: the canvas is drawn at integer scale with smoothing off.
import { SKIN_TEX, playerParts, faceUv, isLegacySkin, type Face } from './skin3d';

export interface FlatRect {
  /** source rect in skin pixels (64x64 layout) */
  src: readonly [number, number, number, number];
  /** destination in model pixels, 16 wide x 32 tall figure */
  dst: readonly [number, number];
  mirror: boolean;
  overlay: boolean;
}

/** Where each part's visible face sits in the 16x32 figure (x from the viewer's left, y from the top). */
function layout(slim: boolean, side: 'front' | 'back'): Record<string, readonly [number, number]> {
  const aw = slim ? 3 : 4;
  // front view: the player's right arm is on the viewer's left; back view mirrors that
  const left = side === 'front' ? 'rightArm' : 'leftArm';
  const right = side === 'front' ? 'leftArm' : 'rightArm';
  const lLeg = side === 'front' ? 'rightLeg' : 'leftLeg';
  const rLeg = side === 'front' ? 'leftLeg' : 'rightLeg';
  const o: Record<string, readonly [number, number]> = {
    head: [4, 0], body: [4, 8],
    [left]: [4 - aw, 8], [right]: [12, 8],
    [lLeg]: [4, 20], [rLeg]: [8, 20],
  };
  o.hat = o.head; o.jacket = o.body;
  o[`${left.replace('Arm', 'Sleeve')}`] = o[left]; o[`${right.replace('Arm', 'Sleeve')}`] = o[right];
  o[`${lLeg.replace('Leg', 'Pants')}`] = o[lLeg]; o[`${rLeg.replace('Leg', 'Pants')}`] = o[rLeg];
  return o;
}

/** All rectangles of one side, base layer first, then the overlay layer. Legacy (64x32) skins mirror the right limbs into the left. */
export function flatRects(slim: boolean, side: 'front' | 'back', legacy = false): FlatRect[] {
  const pos = layout(slim, side);
  const face: Face = side;
  const out: FlatRect[] = [];
  for (const overlayPass of [false, true]) {
    for (const p of playerParts(slim)) {
      if (p.overlay !== overlayPass) continue;
      if (legacy && p.overlay && p.name !== 'hat') continue; // legacy skins only have the hat as second layer
      const [w, h, d] = p.size;
      let [u, v] = p.uv;
      let mirror = false;
      if (legacy && /^left/.test(p.name)) { // use the right limb's pixels, flipped
        const r = playerParts(slim).find((q) => q.name === p.name.replace('left', 'right'));
        if (r) { [u, v] = r.uv; mirror = true; }
      }
      const [sx, sy, sw, sh] = faceUv(face, w, h, d, u, v);
      const dst = pos[p.name];
      if (!dst) continue;
      out.push({ src: [sx, sy, sw, sh], dst, mirror, overlay: p.overlay });
    }
  }
  return out;
}

/** Figure size in model pixels. */
export const FIGURE = { w: 16, h: 32 } as const;
/** The cape seen from behind: outer face (1,1) 10x16 of the 64x32 cape texture, placed behind the figure (x 3..13, y 8..24). */
export const CAPE_BACK = { src: [1, 1, 10, 16] as const, dst: [3, 8] as const };

type Src = CanvasImageSource & { width: number; height: number };

/**
 * Draws the figure into `ctx` at `scale`. HD skins (128, 256 ...) work: coordinates are multiplied by width / 64. `cape` is only drawn
 * on the back view. The caller sizes the canvas (FIGURE.w * scale x FIGURE.h * scale).
 */
export function drawFlat(ctx: CanvasRenderingContext2D, skin: Src, opts: { slim: boolean; side: 'front' | 'back'; scale: number; cape?: Src | null }): void {
  const { slim, side, scale, cape } = opts;
  const k = skin.width / SKIN_TEX[0];
  const legacy = isLegacySkin(skin.width, skin.height);
  ctx.imageSmoothingEnabled = false;
  ctx.clearRect(0, 0, FIGURE.w * scale, FIGURE.h * scale);
  const rects = flatRects(slim, side, legacy);
  const put = (r: FlatRect) => {
    const [sx, sy, sw, sh] = r.src;
    const [dx, dy] = r.dst;
    if (r.mirror) {
      ctx.save(); ctx.translate((dx + sw) * scale, dy * scale); ctx.scale(-1, 1);
      ctx.drawImage(skin, sx * k, sy * k, sw * k, sh * k, 0, 0, sw * scale, sh * scale);
      ctx.restore();
    } else ctx.drawImage(skin, sx * k, sy * k, sw * k, sh * k, dx * scale, dy * scale, sw * scale, sh * scale);
  };
  for (const r of rects) if (!r.overlay) put(r);
  if (side === 'back' && cape) {
    const c = cape.width / 64;
    ctx.drawImage(cape, CAPE_BACK.src[0] * c, CAPE_BACK.src[1] * c, CAPE_BACK.src[2] * c, CAPE_BACK.src[3] * c, CAPE_BACK.dst[0] * scale, CAPE_BACK.dst[1] * scale, 10 * scale, 16 * scale);
  }
  for (const r of rects) if (r.overlay) put(r);
}

/** Cape thumbnail: the outer face of the cape texture (10x16), drawn at integer scale. */
export function drawCapeFront(ctx: CanvasRenderingContext2D, cape: Src, scale: number): void {
  const c = cape.width / 64;
  ctx.imageSmoothingEnabled = false;
  ctx.clearRect(0, 0, 10 * scale, 16 * scale);
  ctx.drawImage(cape, 1 * c, 1 * c, 10 * c, 16 * c, 0, 0, 10 * scale, 16 * scale);
}
