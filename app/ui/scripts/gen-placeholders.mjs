// TEMPORARY placeholder art generator. Uses only node built-ins.
// Output is replaced by real illustrated assets later (see src/assets/fallbacks/README.md).
import { deflateSync } from 'node:zlib';
import { mkdirSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const out = join(dirname(fileURLToPath(import.meta.url)), '..', 'src', 'assets', 'fallbacks');
mkdirSync(out, { recursive: true });

const crcTable = Array.from({ length: 256 }, (_, n) => {
  let c = n;
  for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
  return c >>> 0;
});
const crc32 = (buf) => {
  let c = 0xffffffff;
  for (const b of buf) c = crcTable[(c ^ b) & 0xff] ^ (c >>> 8);
  return (c ^ 0xffffffff) >>> 0;
};
const chunk = (type, data) => {
  const len = Buffer.alloc(4);
  len.writeUInt32BE(data.length);
  const body = Buffer.concat([Buffer.from(type, 'ascii'), data]);
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(body));
  return Buffer.concat([len, body, crc]);
};
function png(w, h, px) {
  const raw = Buffer.alloc((w * 4 + 1) * h);
  for (let y = 0; y < h; y++) {
    raw[y * (w * 4 + 1)] = 0;
    for (let x = 0; x < w; x++) {
      const [r, g, b, a = 255] = px(x, y);
      const o = y * (w * 4 + 1) + 1 + x * 4;
      raw[o] = r; raw[o + 1] = g; raw[o + 2] = b; raw[o + 3] = a;
    }
  }
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(w, 0); ihdr.writeUInt32BE(h, 4);
  ihdr[8] = 8; ihdr[9] = 6;
  return Buffer.concat([
    Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]),
    chunk('IHDR', ihdr), chunk('IDAT', deflateSync(raw)), chunk('IEND', Buffer.alloc(0)),
  ]);
}
// deterministic PRNG
const rng = (seed) => () => ((seed = (Math.imul(seed, 1664525) + 1013904223) >>> 0) / 4294967296);
const hex = (s) => [1, 3, 5].map((i) => parseInt(s.slice(i, i + 2), 16));
const shade = (c, f) => c.map((v) => Math.max(0, Math.min(255, Math.round(v * f))));

const biomes = [
  ['plains', '#6fa343'], ['forest', '#3f7a3a'], ['desert', '#d6c27a'], ['taiga', '#4d6b5a'],
  ['snow', '#dfe8ee'], ['swamp', '#4f6b3e'], ['jungle', '#2f8f3a'], ['savanna', '#b8a24a'],
  ['badlands', '#c0652f'], ['mesa', '#a5532e'], ['cherry', '#e6a3b8'], ['stone', '#7d7f84'],
  ['deepslate', '#3b3d42'], ['nether', '#8a2f2f'], ['end', '#d9d8a0'], ['ocean', '#2e6fa8'],
];
const tile = (base, seed, size = 16) => {
  const r = rng(seed);
  const cells = Array.from({ length: size * size }, () => shade(base, 0.72 + r() * 0.5));
  return (x, y) => cells[y * size + x];
};
biomes.forEach(([, c], i) => {
  writeFileSync(join(out, `placeholder-profile-${i}.png`), png(16, 16, tile(hex(c), 100 + i)));
});
[['#8a6b4a', 1], ['#5b7fa8', 2], ['#7a8f5a', 3]].forEach(([c, s], i) => {
  const base = hex(c), r = rng(500 + s);
  const cells = Array.from({ length: 256 }, () => shade(base, 0.8 + r() * 0.4));
  writeFileSync(join(out, `placeholder-mod-${i}.png`), png(16, 16, (x, y) => {
    const border = x === 0 || y === 0 || x === 15 || y === 15;
    const jar = x >= 5 && x <= 10 && y >= 4 && y <= 12;
    return border ? shade(base, 0.45) : jar ? [255, 173, 50] : cells[y * 16 + x];
  }));
});
{
  const r = rng(900);
  writeFileSync(join(out, 'placeholder-server-0.png'), png(16, 16, (x, y) => {
    const border = x === 0 || y === 0 || x === 15 || y === 15;
    const slot = y % 5 === 2 && x > 2 && x < 13;
    const led = slot && x === 11;
    return led ? [134, 184, 107] : border ? [28, 31, 36] : slot ? [20, 22, 26] : shade([58, 62, 70], 0.9 + r() * 0.2);
  }));
}
// Steve-ish head, 8x8
{
  const skin = [198, 134, 98], hair = [72, 48, 30];
  writeFileSync(join(out, 'placeholder-account-steve.png'), png(8, 8, (x, y) => {
    if (y < 2) return hair;
    if (y === 2 && (x < 1 || x > 6)) return hair;
    if (y === 4 && (x === 1 || x === 2)) return [255, 255, 255];
    if (y === 4 && (x === 5 || x === 6)) return [255, 255, 255];
    if (y === 4 && (x === 2 || x === 5)) return [60, 50, 140];
    if (y === 6 && x > 1 && x < 6) return [110, 66, 48];
    return skin;
  }));
}
// wide hero panorama 320x112
{
  const r = rng(77);
  const H = 112, W = 320;
  const heights = Array.from({ length: W / 8 }, (_, i) => 60 + Math.round(Math.sin(i / 3) * 12 + Math.sin(i / 1.3) * 5));
  const noise = Array.from({ length: W * H }, () => r());
  writeFileSync(join(out, 'placeholder-hero.png'), png(W, H, (x, y) => {
    const ground = heights[Math.floor(x / 8)];
    const n = noise[y * W + x];
    if (y < ground) { const t = y / ground; return [Math.round(40 + 90 * t), Math.round(60 + 90 * t), Math.round(100 + 80 * t)]; }
    if (y < ground + 4) return shade([111, 163, 67], 0.8 + n * 0.4);
    return shade([112, 84, 58], 0.7 + n * 0.4);
  }));
}
console.log('placeholders written to', out);
