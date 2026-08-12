'use strict';
const zlib = require('zlib');

/**
 * Builds the fallback player texture at runtime instead of shipping a binary
 * blob. It follows the modern 64x64 skin layout, so the 3D preview and the game
 * agree on where each body part lives.
 */

const W = 64;
const H = 64;

// Classic palette.
const SKIN = [0xc2, 0x8e, 0x62];
const SKIN_DARK = [0xa9, 0x77, 0x50];
const HAIR = [0x3f, 0x2c, 0x1e];
const SHIRT = [0x00, 0xaa, 0xaa];
const SHIRT_DARK = [0x00, 0x8b, 0x8b];
const PANTS = [0x3c, 0x44, 0xa5];
const PANTS_DARK = [0x2f, 0x36, 0x86];
const SHOES = [0x4d, 0x3a, 0x2a];
const EYE_WHITE = [0xf0, 0xf0, 0xf0];
const EYE_IRIS = [0x3b, 0x5d, 0xc9];
const MOUTH = [0x7b, 0x4c, 0x35];

function createSkin() {
  const px = new Uint8Array(W * H * 4); // transparent by default

  const set = (x, y, [r, g, b], a = 255) => {
    if (x < 0 || y < 0 || x >= W || y >= H) return;
    const i = (y * W + x) * 4;
    px[i] = r; px[i + 1] = g; px[i + 2] = b; px[i + 3] = a;
  };

  const rect = (x, y, w, h, color, a = 255) => {
    for (let dy = 0; dy < h; dy++) for (let dx = 0; dx < w; dx++) set(x + dx, y + dy, color, a);
  };

  /**
   * A cuboid's UV cross. Both cap faces sit on the upper row starting at
   * ox+d, and the four side faces run right/front/left/back below them,
   * the same unwrap Minecraft uses.
   */
  const box = (ox, oy, w, d, h, side, front, top, bottom) => {
    rect(ox + d, oy, w, d, top);
    rect(ox + d + w, oy, w, d, bottom || top);
    rect(ox, oy + d, d, h, side);                  // right
    rect(ox + d, oy + d, w, h, front);             // front
    rect(ox + d + w, oy + d, d, h, side);          // left
    rect(ox + d + w + d, oy + d, w, h, side);      // back
  };

  // --- head: 8x8x8 at (0,0). Side faces occupy y=8..16.
  box(0, 0, 8, 8, 8, SKIN_DARK, SKIN, HAIR, SKIN_DARK);
  rect(0, 8, 32, 3, HAIR);       // hairline wraps all four side faces
  rect(24, 8, 8, 8, HAIR);       // back of the head is fully hair
  // face details live on the front face, which starts at x=8, y=8
  rect(9, 12, 2, 1, EYE_WHITE);
  rect(13, 12, 2, 1, EYE_WHITE);
  set(10, 12, EYE_IRIS);
  set(13, 12, EYE_IRIS);
  rect(11, 14, 3, 1, SKIN_DARK); // nose shadow
  rect(11, 15, 2, 1, MOUTH);

  // --- hat layer at (32,0): only the top of the skull, so the face shows.
  box(32, 0, 8, 8, 8, HAIR, HAIR, HAIR, HAIR);
  for (let y = 11; y < 16; y++) {
    for (let x = 32; x < 64; x++) px[(y * W + x) * 4 + 3] = 0;
  }

  // --- body: 8 wide, 4 deep, 12 high at (16,16)
  box(16, 16, 8, 4, 12, SHIRT_DARK, SHIRT, SHIRT, PANTS);

  // --- arms: 4x4x12. Short teal sleeve on top, bare skin below.
  const arm = (ox, oy) => {
    box(ox, oy, 4, 4, 12, SKIN_DARK, SKIN, SKIN, SKIN);
    rect(ox, oy + 4, 16, 4, SHIRT_DARK);   // sleeve across all side faces
    rect(ox + 4, oy + 4, 4, 4, SHIRT);     // brighter on the front face
  };
  arm(40, 16);  // right arm
  arm(32, 48);  // left arm

  // --- legs: 4x4x12, blue trousers with dark shoes at the very bottom.
  const leg = (ox, oy) => {
    box(ox, oy, 4, 4, 12, PANTS_DARK, PANTS, PANTS, SHOES);
    rect(ox, oy + 14, 16, 2, SHOES);       // shoe band at the foot end
  };
  leg(0, 16);   // right leg
  leg(16, 48);  // left leg

  return px;
}

// ---- minimal PNG encoder ---------------------------------------------------

function crc32(buf) {
  let c;
  const table = crc32.table || (crc32.table = (() => {
    const t = new Int32Array(256);
    for (let n = 0; n < 256; n++) {
      c = n;
      for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
      t[n] = c;
    }
    return t;
  })());
  let crc = -1;
  for (let i = 0; i < buf.length; i++) crc = (crc >>> 8) ^ table[(crc ^ buf[i]) & 0xff];
  return (crc ^ -1) >>> 0;
}

function chunk(type, data) {
  const len = Buffer.alloc(4);
  len.writeUInt32BE(data.length);
  const typeBuf = Buffer.from(type, 'ascii');
  const crcBuf = Buffer.alloc(4);
  crcBuf.writeUInt32BE(crc32(Buffer.concat([typeBuf, data])));
  return Buffer.concat([len, typeBuf, data, crcBuf]);
}

function encodePNG(pixels, width, height) {
  // Each scanline is prefixed with filter type 0 (none).
  const raw = Buffer.alloc(height * (width * 4 + 1));
  for (let y = 0; y < height; y++) {
    const off = y * (width * 4 + 1);
    raw[off] = 0;
    Buffer.from(pixels.buffer, y * width * 4, width * 4).copy(raw, off + 1);
  }
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(width, 0);
  ihdr.writeUInt32BE(height, 4);
  ihdr[8] = 8;   // bit depth
  ihdr[9] = 6;   // RGBA
  ihdr[10] = 0;  // deflate
  ihdr[11] = 0;  // adaptive filtering
  ihdr[12] = 0;  // no interlace

  return Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    chunk('IHDR', ihdr),
    chunk('IDAT', zlib.deflateSync(raw, { level: 9 })),
    chunk('IEND', Buffer.alloc(0)),
  ]);
}

let cached = null;
function defaultSkinDataUrl() {
  if (!cached) {
    cached = `data:image/png;base64,${encodePNG(createSkin(), W, H).toString('base64')}`;
  }
  return cached;
}

module.exports = { defaultSkinDataUrl, encodePNG };
