import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

const css = readFileSync(fileURLToPath(new URL('../app.css', import.meta.url)), 'utf8');
const hex = (h: string): [number, number, number] => [1, 3, 5].map((i) => parseInt(h.slice(i, i + 2), 16)) as [number, number, number];
const lin = (c: number) => { const s = c / 255; return s <= 0.03928 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4; };
const lum = ([r, g, b]: number[]) => 0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b);
const ratio = (a: number[], b: number[]) => { const [x, y] = [lum(a), lum(b)].sort((p, q) => q - p); return (x + 0.05) / (y + 0.05); };
const tok = (block: string, n: string) => hex(block.match(new RegExp(`--${n}:\\s*(#[0-9a-fA-F]{6})`))![1]);
const hue = ([r, g, b]: number[]) => { const mx = Math.max(r, g, b), mn = Math.min(r, g, b); return mx === r ? (60 * (g - b)) / (mx - mn) : 0; };

const dark = css.match(/:root \{[\s\S]*?\n\}/)![0];
const light = css.match(/:root\[data-theme='light'\] \{[\s\S]*?\n\}/)![0];
const media = css.match(/@media \(prefers-color-scheme: light\) \{[\s\S]*?\n\}/)![0];

describe('--brand-gold', () => {
  it('is the sampled mango gold in the dark theme with >= 4.5:1 on the dark surfaces', () => {
    expect(dark).toMatch(/--brand-gold:\s*#FBA103/i);
    for (const s of ['ink-0', 'ink-2', 'ink-3']) expect(ratio(tok(dark, 'brand-gold'), tok(dark, s)), s).toBeGreaterThanOrEqual(4.5);
  });
  it('light theme uses a darker variant of the same hue with >= 4.5:1 on the light surfaces', () => {
    const g = tok(light, 'brand-gold');
    expect(lum(g)).toBeLessThan(lum(tok(dark, 'brand-gold')));
    expect(Math.abs(hue(g) - hue(tok(dark, 'brand-gold')))).toBeLessThan(6);
    for (const s of ['ink-0', 'ink-1', 'ink-2', 'ink-3', 'ink-4']) expect(ratio(g, tok(light, s)), s).toBeGreaterThanOrEqual(4.5);
  });
  it('the OS-preference light block matches the data-theme one, and the always-dark stage keeps the dark gold', () => {
    expect(tok(media, 'brand-gold')).toEqual(tok(light, 'brand-gold'));
    expect(css.match(/\.on-stage \{[\s\S]*?\n\}/)![0]).toMatch(/--brand-gold:\s*#FBA103/i);
  });
});
