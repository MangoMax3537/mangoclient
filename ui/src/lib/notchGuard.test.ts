// Regression guard: a CSS border stops at the diagonal cuts of a clip-path, so a notched element with its own border/outline frame shows open
// corners. The shared helpers are `.frame` (ring around a `.notch`, app.css) and `.nb` (1px border + fill drawn by pseudo-elements). Components
// must use them instead of `border:` on a notched element.
import { describe, expect, it } from 'vitest';
import { readdirSync, readFileSync, statSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { join } from 'node:path';

const root = fileURLToPath(new URL('..', import.meta.url));
const walk = (d: string): string[] => readdirSync(d).flatMap((f) => {
  const p = join(d, f);
  return statSync(p).isDirectory() ? (f === 'node_modules' ? [] : walk(p)) : /\.(svelte|css)$/.test(f) ? [p] : [];
});
const files = walk(root);
const BORDER = /(^|[;{\s])border(-top|-right|-bottom|-left)?\s*:\s*(?!0\b|none)/;

interface Rule { sel: string; body: string }
const rules = (src: string): Rule[] => [...src.matchAll(/([^{}]+)\{([^{}]*)\}/g)].map((m) => ({ sel: m[1].trim(), body: m[2] }));

describe('notched elements', () => {
  it('app.css defines the shared .frame helper next to .notch and .nb', () => {
    const css = readFileSync(join(root, 'app.css'), 'utf8');
    expect(css).toMatch(/\.frame::after\s*\{[^}]*evenodd/);
    expect(css).toMatch(/\.nb::before/);
  });
  it('no rule combines clip-path with a border (the border would be cut at the corners)', () => {
    const bad: string[] = [];
    for (const f of files) for (const r of rules(readFileSync(f, 'utf8'))) {
      if (/clip-path\s*:\s*polygon/.test(r.body) && BORDER.test(r.body)) bad.push(`${f.slice(root.length)}: ${r.sel}`);
    }
    expect(bad).toEqual([]);
  });
  it('no component gives a .notch element a border of its own (use .frame or .nb)', () => {
    const bad: string[] = [];
    for (const f of files.filter((x) => x.endsWith('.svelte'))) {
      const src = readFileSync(f, 'utf8');
      const classes = new Set<string>();
      for (const m of src.matchAll(/class="([^"]*)"/g)) {
        const toks = m[1].split(/\s+/);
        if (toks.includes('notch') && !toks.includes('frame')) toks.filter((t) => t && t !== 'notch' && !t.includes('{')).forEach((t) => classes.add(t));
      }
      for (const r of rules(src.slice(src.indexOf('<style')))) {
        if (!BORDER.test(r.body)) continue;
        const last = r.sel.split(',').map((s) => s.trim());
        for (const s of last) {
          const m = s.match(/^\.([\w-]+)(?:\.[\w-]+|:[\w-]+)*$/);
          if (m && classes.has(m[1])) bad.push(`${f.slice(root.length)}: ${s}`);
        }
      }
    }
    expect(bad).toEqual([]);
  });
  it('a selection frame on a card is drawn with --frame, not a pseudo-element border', () => {
    for (const f of ['components/Library.svelte', 'components/MediaCard.svelte']) {
      const src = readFileSync(join(root, f), 'utf8');
      expect(src).toMatch(/class="card notch frame"/);
      expect(src).not.toMatch(/\.sel::after[^}]*border/);
    }
  });
});

describe('Rail account head', () => {
  it('is shown uncut (no notch on the avatar ring or its tile)', () => {
    const src = readFileSync(join(root, 'components/Rail.svelte'), 'utf8');
    const line = src.split('\n').find((l) => l.includes('kind="account"')) ?? '';
    expect(line).not.toMatch(/notch/);
    expect(line).toContain('<span class="ring">');
  });
});

describe('Skins viewer', () => {
  it('stands free: no panel (nb / on-stage) behind the model', () => {
    const src = readFileSync(join(root, 'components/Skins.svelte'), 'utf8');
    const tag = src.match(/<section class="stage[^"]*"/)?.[0] ?? '';
    expect(tag).not.toMatch(/\bnb\b|on-stage/);
  });
});
