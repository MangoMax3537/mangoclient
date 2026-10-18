// Turns the launcher's pixel-art mango (src/assets/icons/64x64.png) into logo.bin for
// the in-game home screen: the art at its native pixel grid, as ARGB ints.
//
// The home screen draws it one GUI pixel per art pixel with plain fills, so it
// is crisp at every GUI scale and needs none of the texture APIs that change
// between Minecraft versions. Run with `node tools/gen-logo.js` from mod/.
const fs = require('fs');
const path = require('path');
const zlib = require('zlib');

const source = path.join(__dirname, '..', '..', 'src', 'assets', 'icons', '64x64.png');
const target = path.join(__dirname, '..', 'src', 'client', 'resources', 'assets', 'mangoconfig', 'logo.bin');

function decode(file) {
	const buf = fs.readFileSync(file);
	let pos = 8;
	let width = 0;
	let height = 0;
	const idat = [];
	while (pos < buf.length) {
		const len = buf.readUInt32BE(pos);
		const type = buf.toString('ascii', pos + 4, pos + 8);
		const data = buf.subarray(pos + 8, pos + 8 + len);
		if (type === 'IHDR') {
			width = data.readUInt32BE(0);
			height = data.readUInt32BE(4);
			if (data[8] !== 8 || data[9] !== 6 || data[12] !== 0) throw new Error('expected 8-bit RGBA, not interlaced');
		} else if (type === 'IDAT') {
			idat.push(data);
		}
		pos += 12 + len;
	}
	const raw = zlib.inflateSync(Buffer.concat(idat));
	const stride = width * 4;
	const out = Buffer.alloc(height * stride);
	for (let y = 0; y < height; y++) {
		const filter = raw[y * (stride + 1)];
		for (let x = 0; x < stride; x++) {
			const v = raw[y * (stride + 1) + 1 + x];
			const a = x >= 4 ? out[y * stride + x - 4] : 0;
			const b = y > 0 ? out[(y - 1) * stride + x] : 0;
			const c = x >= 4 && y > 0 ? out[(y - 1) * stride + x - 4] : 0;
			let p;
			switch (filter) {
				case 0: p = 0; break;
				case 1: p = a; break;
				case 2: p = b; break;
				case 3: p = (a + b) >> 1; break;
				case 4: {
					const e = a + b - c;
					const pa = Math.abs(e - a), pb = Math.abs(e - b), pc = Math.abs(e - c);
					p = pa <= pb && pa <= pc ? a : pb <= pc ? b : c;
					break;
				}
				default: throw new Error('bad filter ' + filter);
			}
			out[y * stride + x] = (v + p) & 0xFF;
		}
	}
	return { width, height, pixel: (x, y) => out.subarray((y * width + x) * 4, (y * width + x) * 4 + 4) };
}

const img = decode(source);

// The art is an upscale of a small pixel grid; find the largest block size
// whose every block is a single colour (allowing for a little resampling noise).
function uniform(size) {
	if (img.width % size || img.height % size) return false;
	for (let by = 0; by < img.height; by += size) {
		for (let bx = 0; bx < img.width; bx += size) {
			const ref = img.pixel(bx + (size >> 1), by + (size >> 1));
			for (let y = by + 1; y < by + size - 1; y++) {
				for (let x = bx + 1; x < bx + size - 1; x++) {
					const p = img.pixel(x, y);
					for (let i = 0; i < 4; i++) if (Math.abs(p[i] - ref[i]) > 24) return false;
				}
			}
		}
	}
	return true;
}

let block = 1;
for (let size = 2; size <= img.width / 16; size++) if (uniform(size)) block = size;

const w = img.width / block;
const h = img.height / block;
const bin = Buffer.alloc(8 + w * h * 4);
bin.writeInt32BE(w, 0);
bin.writeInt32BE(h, 4);
for (let y = 0; y < h; y++) {
	for (let x = 0; x < w; x++) {
		const [r, g, b, a] = img.pixel(x * block + (block >> 1), y * block + (block >> 1));
		bin.writeInt32BE(((a << 24) | (r << 16) | (g << 8) | b) | 0, 8 + (y * w + x) * 4);
	}
}
fs.mkdirSync(path.dirname(target), { recursive: true });
fs.writeFileSync(target, bin);
console.log(`logo ${img.width}x${img.height} -> ${w}x${h} (block ${block})`);
