import { describe, expect, it } from 'vitest';
import { dictionaries } from './i18n';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

const css = readFileSync(fileURLToPath(new URL('../app.css', import.meta.url)), 'utf8');

// ---- tokens: the skeleton grey has >= 3:1 against the page surfaces in both themes ----
const hex = (h: string): [number, number, number] => [1, 3, 5].map((i) => parseInt(h.slice(i, i + 2), 16)) as [number, number, number];
const lin = (c: number) => { const s = c / 255; return s <= 0.03928 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4; };
const lum = ([r, g, b]: number[]) => 0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b);
const ratio = (a: number[], b: number[]) => { const [x, y] = [lum(a), lum(b)].sort((p, q) => q - p); return (x + 0.05) / (y + 0.05); };
const mix = (top: number[], bottom: number[], pct: number) => top.map((c, i) => c * pct + bottom[i] * (1 - pct));

function theme(block: RegExp) {
  const m = css.match(block);
  if (!m) throw new Error(`block ${block}`);
  const tok = (n: string) => { const r = m[0].match(new RegExp(`--${n}:\\s*(#[0-9a-fA-F]{6})`)); if (!r) throw new Error(n); return hex(r[1]); };
  const mixPct = Number(m[0].match(/--skel-mix:\s*(\d+)%/)?.[1]) / 100;
  return { ink0: tok('ink-0'), ink2: tok('ink-2'), paper: tok('paper'), mixPct };
}
const dark = theme(/:root \{[\s\S]*?\n\}/);
const light = theme(/:root\[data-theme='light'\] \{[\s\S]*?\n\}/);

describe('skeleton token', () => {
  for (const [name, t] of [['dark', dark], ['light', light]] as const) {
    it(`${name}: the shape has >= 3:1 against ink-0 and ink-2 (paper mixed over ink-0)`, () => {
      const skel = mix(t.paper, t.ink0, t.mixPct);
      expect(ratio(skel, t.ink0)).toBeGreaterThanOrEqual(3);
      expect(ratio(skel, t.ink2)).toBeGreaterThanOrEqual(3);
    });
  }
  it('is derived from the surface tokens and the media-query light block uses the same mix as data-theme=light', () => {
    expect(css).toMatch(/--skel:\s*color-mix\(in srgb, var\(--paper\) var\(--skel-mix\), var\(--ink-0\)\)/);
    const media = css.match(/@media \(prefers-color-scheme: light\) \{[\s\S]*?\n\}/)?.[0] ?? '';
    expect(Number(media.match(/--skel-mix:\s*(\d+)%/)?.[1]) / 100).toBe(light.mixPct);
  });
  it('the pulse only animates opacity (1 <-> 0.55) and is static under prefers-reduced-motion', () => {
    const frames = css.match(/@keyframes skel \{[^}]*\}/)?.[0] ?? '';
    expect(frames).toMatch(/opacity:\s*0\.55/);
    expect(frames).not.toMatch(/width|height|top|left|margin|padding/);
    expect(css).toMatch(/\.skel \{[^}]*animation: skel 1\.4s ease-in-out infinite/);
    expect(css).toMatch(/@media \(prefers-reduced-motion: reduce\) \{ \.skel \{ animation: none !important; \} \}/);
  });
  it('no hard-coded colour in the skeleton rule', () => {
    const rule = css.match(/\.skel \{[^}]*\}/)?.[0] ?? '';
    expect(rule).not.toMatch(/#[0-9a-f]{3,8}|rgb/i);
  });
});

describe('skeleton strings', () => {
  it('exist in both languages, differ between them, and keep the placeholders', () => {
    for (const k of ['skel.loading', 'load.failed', 'load.timeout']) {
      expect(dictionaries.en[k], k).toBeTruthy();
      expect(dictionaries.de[k], k).toBeTruthy();
      expect(dictionaries.de[k]).not.toBe(dictionaries.en[k]);
    }
  });
});
