import { describe, expect, it } from 'vitest';
import { render } from 'svelte/server';
import Brand from './Brand.svelte';
import PlanRam from './PlanRam.svelte';
import RamControl from './RamControl.svelte';
import Onboarding from './Onboarding.svelte';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dictionaries } from '../lib/i18n';
import { app } from '../lib/stores.svelte';

app.config = { language: 'en' } as typeof app.config;

// eslint-disable-next-line @typescript-eslint/no-explicit-any
const html = (c: unknown, props: Record<string, unknown> = {}) => render(c as any, { props } as any).body;
const limits = { minMb: 1024, maxMb: 14336, totalMb: 16384 };

describe('Brand', () => {
  it('shows the product name next to the real icon, in two sizes', () => {
    const sm = html(Brand, {});
    expect(sm).toContain('Mango Client');
    expect(sm).toContain('class="brand sm');
    expect(html(Brand, { size: 'lg' })).toContain('class="brand lg');
    expect(sm).toMatch(/aria-hidden="true"/);
  });
  it('uses the --brand-gold token for the wordmark', () => {
    const src = readFileSync(fileURLToPath(new URL('./Brand.svelte', import.meta.url)), 'utf8');
    expect(src).toMatch(/\.wm \{ color: var\(--brand-gold\)/);
  });
  it('no dictionary spells the product name MangoClient', () => {
    for (const lang of ['de', 'en'] as const) for (const [k, v] of Object.entries(dictionaries[lang])) expect(v, `${lang}:${k}`).not.toMatch(/MangoClient/);
  });
});

describe('onboarding', () => {
  it('has four steps and no memory step', () => {
    const out = html(Onboarding);
    expect((out.match(/<li/g) ?? []).length).toBe(4);
    expect(out).not.toMatch(/type="range"/);
    expect(Object.keys(dictionaries.en).filter((k) => /^onb\.(ram|step\.ram)/.test(k))).toEqual([]);
  });
});

describe('plan dialogs: RAM line', () => {
  it('says the pack recommended it and names the cap', () => {
    const out = html(PlanRam, { plan: { ram: { recommendedMb: 4096, minMb: 3072, maxMb: 6144, source: 'modrinth' }, appliedRamMb: 4096 } });
    expect(out).toContain('This instance will get 4 GB (recommended by the pack).');
    expect(out).toContain('The pack advises at most 6 GB.');
  });
  it('falls back to the default without a hint', () => {
    const out = html(PlanRam, { plan: { ram: null, appliedRamMb: null } });
    expect(out).toContain('(the default)');
    expect(out).not.toContain('advises');
  });
});

describe('RamControl', () => {
  const props = { id: 'x', value: 8192, source: 'user', hint: { recommendedMb: 4096, minMb: 3072, maxMb: 6144, source: 'modrinth' }, limits, defaultMb: 4096, onsave: () => {} };
  it('renders slider, GB field, source label, both reset buttons and the cap warning', () => {
    const out = html(RamControl, props);
    expect(out).toContain('type="range"');
    expect(out).toContain('min="1024"'); expect(out).toContain('max="14336"'); expect(out).toContain('step="256"');
    expect(out).toContain('Set by you');
    expect(out).toContain('Use default (4 GB)');
    expect(out).toContain('Use pack recommendation (4 GB)');
    expect(out).toContain('garbage collector');
  });
  it('default source: no default button enabled, no pack button without a hint', () => {
    const out = html(RamControl, { ...props, value: 4096, source: 'default', hint: null });
    expect(out).toContain('Default');
    expect(out).not.toContain('Use pack recommendation');
    expect(out).toMatch(/<button[^>]*disabled[^>]*>[^]*Use default/);
  });
  it('global mode offers the suggestion', () => {
    expect(html(RamControl, { ...props, mode: 'global', source: null, hint: null })).toContain('Use suggestion (4 GB)');
  });
});
