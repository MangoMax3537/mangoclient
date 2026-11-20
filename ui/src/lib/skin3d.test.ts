import { describe, expect, it } from 'vitest';
import { CAPE_PART, CAPE_TEX, SKIN_TEX, boxGeometry, faceUv, isLegacySkin, m4, partGeometry, playerParts } from './skin3d';
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
