// Regression guard: the packaged CSP (src-tauri/tauri.conf.json) has `connect-src ipc: http://ipc.localhost`, so fetch() / XHR / createImageBitmap(fetch)
// of images are BLOCKED in the real app (the dev server and a plain browser would not show it). Image, background and texture code must
// load through <img> (img-src) only, and decode() must never be awaited without a ceiling (WebKitGTK can leave it pending forever).
import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

const read = (rel: string): string => readFileSync(fileURLToPath(new URL(rel, import.meta.url)), 'utf8');
const conf = JSON.parse(read('../../../src-tauri/tauri.conf.json')) as { app: { security: { csp: string } } };
const csp = conf.app.security.csp;
const dir = (name: string): string[] => (csp.split(';').map((d) => d.trim().split(/\s+/)).find((d) => d[0] === name) ?? []).slice(1);

describe('packaged CSP', () => {
  it('lets images come from the page origin, data:, blob: and the mangoimg:// scheme', () => {
    const img = dir('img-src');
    for (const s of ["'self'", 'data:', 'blob:', 'mangoimg:']) expect(img).toContain(s);
  });
  it('does NOT allow fetch() of the page origin or mangoimg (connect-src is IPC only)', () => {
    const c = dir('connect-src');
    expect(c).not.toContain("'self'");
    expect(c).not.toContain('mangoimg:');
    expect(c).not.toContain('*');
    expect(c).toContain('ipc:');
  });
  it('has no worker-src / script-src loosening the code relies on (workers fall back to default-src self)', () => {
    expect(csp).not.toMatch(/unsafe-eval/);
  });
});

/** Modules that load backgrounds, wallpapers, icons, panorama faces, skin and cape textures. */
const IMAGE_MODULES = [
  '../components/BgLayer.svelte', '../components/HomeBackground.svelte', '../components/PanoramaBackground.svelte',
  '../components/BackgroundPicker.svelte', '../components/Img.svelte', '../components/BlockIcon.svelte', '../components/IconPicker.svelte',
  '../components/SkinViewer.svelte', '../components/Tile.svelte', './imgLoad.ts', './wallpapers.ts', './bgCycle.ts', './skinTexture.ts',
  './fallbackImage.ts', './instanceIcon.ts',
];
// A justified exception goes here as `path: reason`; none are needed today.
const ALLOW: Record<string, string> = {};
const strip = (src: string): string => src.replace(/\/\*[\s\S]*?\*\//g, '').replace(/(^|[^:])\/\/.*$/gm, '$1');

describe('image / background / texture loaders avoid APIs the packaged CSP blocks', () => {
  for (const m of IMAGE_MODULES) {
    it(`${m.split('/').pop()}: no fetch, XHR, createImageBitmap, EventSource or WebSocket`, () => {
      if (ALLOW[m]) return;
      const code = strip(read(m));
      expect(code).not.toMatch(/\bfetch\s*\(/);
      expect(code).not.toMatch(/XMLHttpRequest/);
      expect(code).not.toMatch(/createImageBitmap/);
      expect(code).not.toMatch(/new\s+(EventSource|WebSocket)\b/);
    });
  }
  it('the only fetch() in the UI sources is the dev/browser HTTP bridge (httpApi.ts, used with ?backend=, never in the packaged shell)', () => {
    const files = ['./httpApi.ts', './skinTexture.ts', './imgLoad.ts', './stores.svelte.ts', './api.ts', './apiFacade.ts'];
    const hits = files.filter((f) => /\bfetch\s*\(/.test(strip(read(f))));
    expect(hits).toEqual(['./httpApi.ts']);
  });
  it('img.decode() is only awaited through decodeSoon (bounded), never directly', () => {
    for (const m of IMAGE_MODULES.filter((x) => x !== './imgLoad.ts')) expect(strip(read(m)), m).not.toMatch(/\.decode\s*\(/);
  });
  it('no CSS url() / blob: / crossorigin pitfalls in the background layer', () => {
    const code = strip(read('../components/BgLayer.svelte'));
    expect(code).not.toMatch(/url\(\s*['"]?(?!#)/);
    expect(code).not.toMatch(/URL\.createObjectURL/);
  });
  it('skin textures use <img crossOrigin="anonymous"> first (mangoimg:// then needs Access-Control-Allow-Origin for the app origin)', () => {
    expect(read('./skinTexture.ts')).toMatch(/crossOrigin = 'anonymous'/);
  });
});
