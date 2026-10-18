// Writes the Colour Saturation post chains, one per 10% step, in each of the
// five formats Minecraft's post effects have used since 1.20.5.
//
// Every chain is vanilla's own color_convolve program followed by a blit back
// to the main target, so nothing here ships a shader of its own. Run with
// `node tools/gen-saturation.js` from mod/ after changing the steps.
const fs = require('fs');
const path = require('path');

const root = path.join(__dirname, '..', 'src', 'fx');
const GRAY = [0.3, 0.59, 0.11];
// From 1.21.5 the program bakes Saturation in as a constant of 1.8 and only
// the colour matrix is still a uniform. A saturation matrix with the same grey
// weights composes by multiplying (S(a) * S(b) = S(ab)), so a matrix of
// S(s / 1.8) brings the result back to exactly s.
const BAKED = 1.8;

const round = (n) => Math.round(n * 1e6) / 1e6;

function matrix(k) {
	return [0, 1, 2].map((row) => GRAY.map((g, col) => round((1 - k) * g + (col === row ? k : 0))));
}

function write(variant, file, json) {
	const out = path.join(root, variant, 'resources', 'assets', 'mangoconfig', file);
	fs.mkdirSync(path.dirname(out), { recursive: true });
	fs.writeFileSync(out, JSON.stringify(json, null, '\t') + '\n');
}

for (let step = 0; step <= 20; step++) {
	const s = step / 10;
	const name = 'saturation_' + String(step * 10).padStart(3, '0');
	const [red, green, blue] = matrix(s / BAKED);

	// a: 1.20.5 - 1.21.1, shaders/post with named programs.
	write('a', `shaders/post/${name}.json`, {
		targets: ['swap'],
		passes: [
			{ name: 'color_convolve', intarget: 'minecraft:main', outtarget: 'swap',
				uniforms: [{ name: 'Saturation', values: [s] }] },
			{ name: 'blit', intarget: 'swap', outtarget: 'minecraft:main' },
		],
	});

	// b: 1.21.2 - 1.21.4, post_effect with program ids.
	write('b', `post_effect/${name}.json`, {
		targets: { swap: {} },
		passes: [
			{ program: 'minecraft:post/color_convolve',
				inputs: [{ sampler_name: 'In', target: 'minecraft:main' }], output: 'swap',
				uniforms: [{ name: 'Saturation', values: [s] }] },
			{ program: 'minecraft:post/blit',
				inputs: [{ sampler_name: 'In', target: 'swap' }], output: 'minecraft:main' },
		],
	});

	// c: 1.21.5, separate shaders and typed uniform lists.
	write('c', `post_effect/${name}.json`, {
		targets: { swap: {} },
		passes: [
			{ vertex_shader: 'minecraft:post/sobel', fragment_shader: 'minecraft:post/color_convolve',
				inputs: [{ sampler_name: 'In', target: 'minecraft:main' }], output: 'swap',
				uniforms: [
					{ name: 'RedMatrix', type: 'vec3', values: red },
					{ name: 'GreenMatrix', type: 'vec3', values: green },
					{ name: 'BlueMatrix', type: 'vec3', values: blue },
				] },
			{ vertex_shader: 'minecraft:post/blit', fragment_shader: 'minecraft:post/blit',
				inputs: [{ sampler_name: 'In', target: 'swap' }],
				uniforms: [{ name: 'ColorModulate', type: 'vec4', values: [1, 1, 1, 1] }],
				output: 'minecraft:main' },
		],
	});

	// d: 1.21.6 - 1.21.8 and e: 1.21.9+, uniform blocks; only the vertex
	// shaders differ, after post/sobel and post/blit were folded into core/screenquad.
	for (const [variant, convolveVsh, blitVsh] of [
		['d', 'minecraft:post/sobel', 'minecraft:post/blit'],
		['e', 'minecraft:core/screenquad', 'minecraft:core/screenquad'],
	]) {
		write(variant, `post_effect/${name}.json`, {
			targets: { swap: {} },
			passes: [
				{ vertex_shader: convolveVsh, fragment_shader: 'minecraft:post/color_convolve',
					inputs: [{ sampler_name: 'In', target: 'minecraft:main' }], output: 'swap',
					uniforms: { ColorConfig: [
						{ name: 'RedMatrix', type: 'vec3', value: red },
						{ name: 'GreenMatrix', type: 'vec3', value: green },
						{ name: 'BlueMatrix', type: 'vec3', value: blue },
					] } },
				{ vertex_shader: blitVsh, fragment_shader: 'minecraft:post/blit',
					inputs: [{ sampler_name: 'In', target: 'swap' }],
					uniforms: { BlitConfig: [{ name: 'ColorModulate', type: 'vec4', value: [1, 1, 1, 1] }] },
					output: 'minecraft:main' },
			],
		});
	}
}
