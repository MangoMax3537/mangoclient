import { describe, expect, it } from 'vitest';
import { CAPE_PART, CAPE_TEX, SKIN_TEX, WALK_SWING, boxGeometry, capeMatrix, faceUv, isLegacySkin, m4, partGeometry, partMatrix, playerParts, swingAngle } from './skin3d';
import { FIGURE, flatRects } from './skinFlat';

describe('boxGeometry', () => {
  it('a cuboid has 6 faces: 24 vertices, 36 indices', () => {
    const g = boxGeometry(8, 8, 8, 0, 0);
    expect(g.pos.length).toBe(24 * 3); expect(g.uvs.length).toBe(24 * 2); expect(g.norms.length).toBe(24 * 3); expect(g.idx.length).toBe(36);
    expect(Math.max(...g.idx)).toBe(23);
  });
  it('inflate grows the box by the amount on every side', () => {
    const g = boxGeometry(8, 8, 8, 0, 0, 0.5);
    expect(Math.max(...g.pos)).toBeCloseTo(4.5, 5); expect(Math.min(...g.pos)).toBeCloseTo(-4.5, 5);
  });
  it('every UV of every part stays inside 0..1 (classic and slim)', () => {
    for (const slim of [false, true]) for (const p of [...playerParts(slim), CAPE_PART]) {
      for (const v of partGeometry(p).uvs) { expect(v).toBeGreaterThanOrEqual(0); expect(v).toBeLessThanOrEqual(1); }
    }
  });
  it('the head front face maps to pixels (8,8)-(16,16) of the 64x64 skin', () => {
    expect(faceUv('front', 8, 8, 8, 0, 0)).toEqual([8, 8, 8, 8]);
    const g = boxGeometry(8, 8, 8, 0, 0);
    // face 0 = front: corners (u0,v1) (u1,v1) (u1,v0) (u0,v0) in texture space
    expect(g.uvs.slice(0, 8).map((x) => x * 64)).toEqual([8, 16, 16, 16, 16, 8, 8, 8]);
  });
  it('the unwrap of a 4x12x4 limb: front 4 wide at u+4, right/left sides 4 wide, top above', () => {
    expect(faceUv('front', 4, 12, 4, 40, 16)).toEqual([44, 20, 4, 12]);
    expect(faceUv('back', 4, 12, 4, 40, 16)).toEqual([52, 20, 4, 12]);
    expect(faceUv('right', 4, 12, 4, 40, 16)).toEqual([40, 20, 4, 12]);
    expect(faceUv('left', 4, 12, 4, 40, 16)).toEqual([48, 20, 4, 12]);
    expect(faceUv('top', 4, 12, 4, 40, 16)).toEqual([44, 16, 4, 4]);
  });
});

describe('player parts', () => {
  it('12 parts: 6 base + 6 overlay, the overlay layer is inflated', () => {
    for (const slim of [false, true]) {
      const parts = playerParts(slim);
      expect(parts).toHaveLength(12);
      expect(parts.filter((p) => p.overlay)).toHaveLength(6);
      expect(parts.filter((p) => p.overlay).every((p) => p.inflate > 0)).toBe(true);
      expect(parts.filter((p) => !p.overlay).every((p) => p.inflate === 0)).toBe(true);
      expect(parts.map((p) => p.name).sort()).toEqual(['body', 'hat', 'head', 'jacket', 'leftArm', 'leftLeg', 'leftPants', 'leftSleeve', 'rightArm', 'rightLeg', 'rightPants', 'rightSleeve']);
    }
  });
  it('classic arms are 4 wide, slim arms 3 wide and half a unit closer to the body', () => {
    const arm = (slim: boolean, n: string) => playerParts(slim).find((p) => p.name === n)!;
    expect(arm(false, 'rightArm').size[0]).toBe(4); expect(arm(true, 'rightArm').size[0]).toBe(3);
    expect(arm(false, 'leftSleeve').size[0]).toBe(4); expect(arm(true, 'leftSleeve').size[0]).toBe(3);
    expect(arm(false, 'rightArm').pivot[0]).toBe(-6); expect(arm(true, 'rightArm').pivot[0]).toBe(-5.5);
    expect(arm(false, 'leftArm').pivot[0]).toBe(6); expect(arm(true, 'leftArm').pivot[0]).toBe(5.5);
    // geometry reflects the width: x extent of the slim arm is 3
    const xs = partGeometry(arm(true, 'rightArm')).pos.filter((_, i) => i % 3 === 0);
    expect(Math.max(...xs) - Math.min(...xs)).toBe(3);
  });
  it('vertex counts are identical for classic and slim (only widths differ)', () => {
    expect(playerParts(false).map((p) => partGeometry(p).pos.length)).toEqual(playerParts(true).map((p) => partGeometry(p).pos.length));
  });
  it('overlay UV origins are the second-layer slots of the 64x64 skin', () => {
    const uv = (n: string) => playerParts(false).find((p) => p.name === n)!.uv;
    expect(uv('hat')).toEqual([32, 0]); expect(uv('jacket')).toEqual([16, 32]); expect(uv('rightSleeve')).toEqual([40, 32]);
    expect(uv('leftSleeve')).toEqual([48, 48]); expect(uv('rightPants')).toEqual([0, 32]); expect(uv('leftPants')).toEqual([0, 48]);
  });
  it('the cape is 10x16x1 on a 64x32 texture with the standard unwrap', () => {
    expect(CAPE_PART.size).toEqual([10, 16, 1]); expect(CAPE_PART.tex).toEqual(CAPE_TEX); expect(SKIN_TEX).toEqual([64, 64]);
    expect(faceUv('front', 10, 16, 1, 0, 0)).toEqual([1, 1, 10, 16]); // outer face
    expect(faceUv('back', 10, 16, 1, 0, 0)).toEqual([12, 1, 10, 16]); // inner face
    const g = partGeometry(CAPE_PART);
    expect(Math.max(...g.uvs.filter((_, i) => i % 2 === 1))).toBeLessThanOrEqual(17 / 32 + 1e-9); // v only uses the top half of the 32 px texture
  });
});

describe('legacy skins and maths', () => {
  it('64x32 (and HD 128x64) are legacy; square skins are not', () => {
    expect(isLegacySkin(64, 32)).toBe(true); expect(isLegacySkin(128, 64)).toBe(true); expect(isLegacySkin(64, 64)).toBe(false); expect(isLegacySkin(32, 16)).toBe(false);
  });
  it('mat4 translate / rotate compose', () => {
    const m = m4.translate(m4.identity(), 1, 2, 3);
    expect([m[12], m[13], m[14]]).toEqual([1, 2, 3]);
    const r = m4.rotateY(m4.identity(), Math.PI / 2);
    expect(r[0]).toBeCloseTo(0, 5); expect(r[8]).toBeCloseTo(1, 5);
  });
});

describe('flat fallback rectangles', () => {
  it('front view: all rects stay inside the 16x32 figure and 64x64 skin; overlays come last', () => {
    for (const slim of [false, true]) for (const side of ['front', 'back'] as const) {
      const rs = flatRects(slim, side);
      expect(rs.length).toBe(12);
      const firstOverlay = rs.findIndex((r) => r.overlay);
      expect(rs.slice(firstOverlay).every((r) => r.overlay)).toBe(true);
      for (const r of rs) {
        expect(r.dst[0]).toBeGreaterThanOrEqual(0); expect(r.dst[0] + r.src[2]).toBeLessThanOrEqual(FIGURE.w);
        expect(r.dst[1] + r.src[3]).toBeLessThanOrEqual(FIGURE.h);
        expect(r.src[0] + r.src[2]).toBeLessThanOrEqual(64); expect(r.src[1] + r.src[3]).toBeLessThanOrEqual(64);
      }
    }
  });
  it('slim arms are 3 px wide; the front head is the (8,8) face', () => {
    expect(flatRects(true, 'front').filter((r) => !r.overlay && r.src[3] === 12 && r.src[2] === 3)).toHaveLength(2);
    expect(flatRects(false, 'front').find((r) => r.dst[0] === 4 && r.dst[1] === 0 && !r.overlay)!.src).toEqual([8, 8, 8, 8]);
    expect(flatRects(false, 'back').find((r) => r.dst[1] === 0 && !r.overlay)!.src).toEqual([24, 8, 8, 8]);
  });
  it('legacy skins mirror the right limbs into the left ones and have no second layer except the hat', () => {
    const rs = flatRects(false, 'front', true);
    expect(rs.filter((r) => r.mirror)).toHaveLength(2);
    expect(rs.filter((r) => r.overlay)).toHaveLength(1);
  });
});

/* ---- pose maths: what is drawn in the Walking pose ---- */
type Mat = ReturnType<typeof m4.identity>;
const apply = (m: Mat, v: readonly number[]): number[] => [
  m[0] * v[0] + m[4] * v[1] + m[8] * v[2] + m[12], m[1] * v[0] + m[5] * v[1] + m[9] * v[2] + m[13], m[2] * v[0] + m[6] * v[1] + m[10] * v[2] + m[14],
];
const det3 = (m: Mat): number => m[0] * (m[5] * m[10] - m[9] * m[6]) - m[4] * (m[1] * m[10] - m[9] * m[2]) + m[8] * (m[1] * m[6] - m[5] * m[2]);
const root = (rot = 0.5, pitch = 0.2): Mat => m4.rotateX(m4.rotateY(m4.identity(), rot), pitch);
const PHASES = [0, 0.13, 0.26, 0.39, 0.52, 0.65, 0.78, 1.05];

describe('walking pose geometry', () => {
  it('every model matrix is a proper rotation + translation (no mirroring, so no face flips)', () => {
    for (const slim of [false, true]) for (const t of PHASES) for (const p of playerParts(slim)) {
      expect(det3(partMatrix(p.name, p.pivot, root(), t, true))).toBeCloseTo(1, 4);
    }
    expect(det3(capeMatrix(root(), 0.4, true, true))).toBeCloseTo(1, 4);
  });
  it('every triangle of every part still faces outward after the limb rotation (counter-clockwise seen from outside)', () => {
    for (const slim of [false, true]) for (const t of PHASES) for (const walking of [false, true]) {
      for (const p of [...playerParts(slim), CAPE_PART]) {
        const g = partGeometry(p);
        const m = p.name === 'cape' ? capeMatrix(root(), t, walking, true) : partMatrix(p.name, p.pivot, root(), t, walking);
        const c = apply(m, [0, 0, 0]);
        for (let i = 0; i < g.idx.length; i += 3) {
          const [a, b, d] = [0, 1, 2].map((k) => apply(m, g.pos.slice(g.idx[i + k] * 3, g.idx[i + k] * 3 + 3)));
          const u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]], v = [d[0] - a[0], d[1] - a[1], d[2] - a[2]];
          const n = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]];
          const mid = [(a[0] + b[0] + d[0]) / 3 - c[0], (a[1] + b[1] + d[1]) / 3 - c[1], (a[2] + b[2] + d[2]) / 3 - c[2]];
          expect(n[0] * mid[0] + n[1] * mid[1] + n[2] * mid[2]).toBeGreaterThan(0);
        }
      }
    }
  });
  it('arms and legs swing about the shoulder / hip, at most about 35 degrees, opposite phases', () => {
    expect(WALK_SWING).toBeGreaterThan(0.55); expect(WALK_SWING).toBeLessThan(0.66);
    const t = Math.PI / 12; // sin(6t) = 1: full swing
    const I = m4.identity();
    const pt = (name: string, v: number[]) => apply(partMatrix(name as never, playerParts(false).find((q) => q.name === name)!.pivot, I, t, true), v);
    // the shoulder joint (top of the arm) and the hip stay where they are, the hand and the foot move
    const shoulder = pt('rightArm', [0, 6, 0]), hand = pt('rightArm', [0, -6, 0]);
    expect(shoulder[1]).toBeCloseTo(24 + Math.abs(Math.sin(6 * t)) * 0.5, 4); expect(shoulder[2]).toBeCloseTo(0, 4);
    expect(Math.abs(hand[2])).toBeGreaterThan(5);
    const hip = pt('rightLeg', [0, 6, 0]);
    expect(hip[1]).toBeCloseTo(12 + 0.5, 4);
    // right arm vs left arm and right arm vs right leg swing in opposite directions
    const lh = pt('leftArm', [0, -6, 0]), rf = pt('rightLeg', [0, -6, 0]);
    expect(Math.sign(hand[2])).toBe(-Math.sign(lh[2]));
    expect(Math.sign(hand[2])).toBe(-Math.sign(rf[2]));
    expect(swingAngle(t, true)).toBeCloseTo(WALK_SWING, 5);
    // idle: legs stay put, arms only breathe
    expect(Math.abs(swingAngle(1, false))).toBeLessThan(0.08);
  });
  it('the cape hangs behind the body and its outer face looks backwards', () => {
    const m = capeMatrix(m4.identity(), 0, false, true);
    const front = apply(m, [0, 0, 1]), back = apply(m, [0, 0, -1]);
    expect(apply(m, [0, 0, 0])[2]).toBeLessThan(-2);
    expect(front[2]).toBeLessThan(back[2]); // the cuboid's front (+z) now points to -z
  });
  it('UV rectangles fit the texture for classic, slim and legacy (64x32) skins; the legacy expansion keeps the unwrap of the 64x64 slots', () => {
    for (const slim of [false, true]) for (const p of playerParts(slim)) {
      const g = partGeometry(p);
      expect(g.uvs.every((x) => x >= 0 && x <= 1)).toBe(true);
    }
    expect(isLegacySkin(64, 32)).toBe(true); expect(isLegacySkin(64, 64)).toBe(false); expect(isLegacySkin(128, 64)).toBe(true);
    // right-leg / right-arm slots exist in a 64x32 skin (v < 32), the left-hand slots (v >= 48) come from the mirror expansion
    const parts = playerParts(false);
    expect(parts.find((q) => q.name === 'rightLeg')!.uv[1] + 16).toBeLessThanOrEqual(32);
    expect(parts.find((q) => q.name === 'leftLeg')!.uv[1]).toBe(48);
  });
});
