// Minimal PNG decoder (8-bit, colour type 2/6, non-interlaced; what WebKit's screenshot returns) plus pixel statistics.
// Used to judge a WebGL canvas from the composited screenshot, since a canvas without preserveDrawingBuffer cannot be read back reliably.
import { inflateSync } from 'node:zlib';

export function decodePng(buf) {
  if (buf.readUInt32BE(0) !== 0x89504e47) throw new Error('not a PNG');
  let off = 8;
  let w = 0, h = 0, depth = 0, ctype = 0, interlace = 0;
  const idat = [];
  while (off < buf.length) {
    const len = buf.readUInt32BE(off);
    const type = buf.toString('latin1', off + 4, off + 8);
    const data = buf.subarray(off + 8, off + 8 + len);
    if (type === 'IHDR') {
      w = data.readUInt32BE(0); h = data.readUInt32BE(4); depth = data[8]; ctype = data[9]; interlace = data[12];
    } else if (type === 'IDAT') idat.push(data);
    else if (type === 'IEND') break;
    off += 12 + len;
  }
  if (depth !== 8 || (ctype !== 2 && ctype !== 6) || interlace) throw new Error(`unsupported PNG (depth ${depth}, colour type ${ctype}, interlace ${interlace})`);
  const bpp = ctype === 6 ? 4 : 3;
  const raw = inflateSync(Buffer.concat(idat));
  const stride = w * bpp;
  const out = Buffer.alloc(w * h * 4);
  let prev = Buffer.alloc(stride);
  let p = 0;
  for (let y = 0; y < h; y++) {
    const f = raw[p++];
    const line = Buffer.from(raw.subarray(p, p + stride));
    p += stride;
    for (let i = 0; i < stride; i++) {
      const a = i >= bpp ? line[i - bpp] : 0, b = prev[i], c = i >= bpp ? prev[i - bpp] : 0;
      let v = line[i];
      if (f === 1) v += a; else if (f === 2) v += b; else if (f === 3) v += (a + b) >> 1;
      else if (f === 4) { const pa = Math.abs(b - c), pb = Math.abs(a - c), pc = Math.abs(a + b - 2 * c); v += pa <= pb && pa <= pc ? a : pb <= pc ? b : c; }
      line[i] = v & 255;
    }
    for (let x = 0; x < w; x++) {
      const s = x * bpp, d = (y * w + x) * 4;
      out[d] = line[s]; out[d + 1] = line[s + 1]; out[d + 2] = line[s + 2]; out[d + 3] = bpp === 4 ? line[s + 3] : 255;
    }
    prev = line;
  }
  return { w, h, data: out };
}

/** Statistics of a rectangle (device pixels): dominant colour, share of pixels that differ from it, number of distinct coarse colours. */
export function regionStats(img, r) {
  const x0 = Math.max(0, Math.floor(r.x)), y0 = Math.max(0, Math.floor(r.y));
  const x1 = Math.min(img.w, Math.ceil(r.x + r.w)), y1 = Math.min(img.h, Math.ceil(r.y + r.h));
  const hist = new Map();
  let n = 0;
  for (let y = y0; y < y1; y++) for (let x = x0; x < x1; x++) {
    const i = (y * img.w + x) * 4;
    const k = ((img.data[i] >> 4) << 8) | ((img.data[i + 1] >> 4) << 4) | (img.data[i + 2] >> 4);
    hist.set(k, (hist.get(k) || 0) + 1);
    n++;
  }
  if (!n) return { n: 0, dominant: null, otherShare: 0, distinct: 0, significant: 0 };
  let best = -1, bestN = 0;
  for (const [k, c] of hist) if (c > bestN) { best = k; bestN = c; }
  // colours that cover at least 0.2 % of the area (ignores anti-aliasing dust)
  const significant = [...hist.values()].filter((c) => c / n >= 0.002).length;
  const hex = (k) => `#${[(k >> 8) & 15, (k >> 4) & 15, k & 15].map((v) => (v * 17).toString(16).padStart(2, '0')).join('')}`;
  return { n, dominant: hex(best), otherShare: 1 - bestN / n, distinct: hist.size, significant };
}

/** Mean absolute difference of two equally sized rectangles of two images, 0..255 (for "did this area change"). */
export function meanDiff(a, b) {
  if (a.w !== b.w || a.h !== b.h) return 255;
  let s = 0;
  for (let i = 0; i < a.data.length; i += 4) s += Math.abs(a.data[i] - b.data[i]) + Math.abs(a.data[i + 1] - b.data[i + 1]) + Math.abs(a.data[i + 2] - b.data[i + 2]);
  return s / ((a.data.length / 4) * 3);
}
