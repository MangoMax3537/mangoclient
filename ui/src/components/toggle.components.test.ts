import { describe, expect, it } from 'vitest';
import { render } from 'svelte/server';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import Toggle from './Toggle.svelte';
import SettingRow from './SettingRow.svelte';

// eslint-disable-next-line @typescript-eslint/no-explicit-any
const html = (c: unknown, props: Record<string, unknown> = {}) => render(c as any, { props } as any).body;
const src = (f: string) => readFileSync(fileURLToPath(new URL(f, import.meta.url)), 'utf8');

describe('Toggle', () => {
  it('is a real checkbox with role=switch and a name', () => {
    const out = html(Toggle, { checked: true, onchange: () => {}, label: 'Telemetry' });
    expect(out).toMatch(/<input[^>]*type="checkbox"/);
    expect(out).toMatch(/role="switch"/);
    expect(out).toMatch(/aria-label="Telemetry"/);
    expect(out).toMatch(/checked/);
  });
  it('the whole 44x24 switch is the hit target: the input covers the track edge to edge', () => {
    const css = src('./Toggle.svelte');
    expect(css).toMatch(/\.toggle \{[^}]*width: 44px; height: 24px/);
    expect(css).toMatch(/input \{[^}]*position: absolute; inset: 0; width: 100%; height: 100%/);
    expect(css).toMatch(/onkeydown=\{\(e\) => \{ if \(e\.key === 'Enter'\)/); // Space is native, Enter is added
  });
  it('no other checkbox-switch markup is left: every switch goes through Toggle', () => {
    const files = import.meta.glob('./**/*.svelte', { eager: true, query: '?raw', import: 'default' }) as Record<string, string>;
    for (const [f, s] of Object.entries(files)) if (!f.endsWith('Toggle.svelte')) expect(s, f).not.toMatch(/class="switch"/);
    expect(src('../app.css')).not.toMatch(/\.switch\b/);
  });
});

describe('SettingRow', () => {
  it('a toggle row is one <label>, so a click on its text toggles; the row carries its search id', () => {
    const out = html(SettingRow, { id: 'telemetry', label: 'Ping', hint: 'h', asLabel: true });
    expect(out).toMatch(/<label[^>]*class="set-row[^"]*"[^>]*id="set-telemetry"|<label[^>]*id="set-telemetry"[^>]*class="set-row/);
  });
});

describe('one hairline between neighbours', () => {
  it('separators are drawn only as adjacent-sibling top borders, never as row/section bottom borders', () => {
    const css = src('../app.css');
    // sections are separated by whitespace only: a gap, no rule between them
    expect(css).toMatch(/\.set-sections \{ display: grid; gap: (4[0-9]|5[0-6])px; \}/);
    expect(css).not.toMatch(/\.set-sections > \* \+ \*/);
    expect(css).toMatch(/\.set-rows > \* \+ \* \{ border-top/);
    for (const f of ['./Settings.svelte', './profile/ProfileSettings.svelte', './SettingSection.svelte', './SettingRow.svelte']) {
      expect(src(f), f).not.toMatch(/border-bottom/);
    }
  });
});
