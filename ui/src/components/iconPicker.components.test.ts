import { describe, expect, it } from 'vitest';
import { render } from 'svelte/server';
import IconPicker from './IconPicker.svelte';
import { en7, de7 } from '../lib/i18n7';

// eslint-disable-next-line @typescript-eslint/no-explicit-any
const html = (props: Record<string, unknown>) => render(IconPicker as any, { props } as any).body;

describe('IconPicker', () => {
  it('offers the MangoClient icon as a tile next to Auto, even before the block gallery is prepared', () => {
    const out = html({ profileId: null, seed: 's', onclose: () => {} });
    expect(out).toContain('role="radiogroup"');
    const radios = out.match(/role="radio"/g) ?? [];
    expect(radios.length).toBeGreaterThanOrEqual(2);
    expect(out).toContain('Mango Client');
  });
  it('marks it checked when it is the initial choice', () => {
    const out = html({ profileId: null, seed: 's', initial: { kind: 'block', block: 'mangoclient' }, onclose: () => {} });
    expect(out).toMatch(/aria-checked="true"[^>]*>[\s\S]*?Mango Client/);
  });
  it('has the label in EN and DE', () => {
    expect(en7['icons.brand']).toBe('Mango Client');
    expect(de7['icons.brand']).toBe('Mango Client');
  });
});
