import { describe, expect, it } from 'vitest';
import { cardHandlers } from './instanceCard';

function setup(selected = false) {
  const log: string[] = [];
  const h = cardHandlers('p1', { select: (id) => log.push(`select:${id}`), open: (id) => log.push(`open:${id}`), isSelected: () => selected });
  return { log, h };
}
describe('instance card clicks', () => {
  it('a single click selects and does not open', () => { const { log, h } = setup(); h.onclick({ detail: 1 }); expect(log).toEqual(['select:p1']); });
  it('an already selected card is not selected again', () => { const { log, h } = setup(true); h.onclick({ detail: 1 }); expect(log).toEqual([]); });
  it('double click: the second click does nothing, dblclick opens once', () => {
    const { log, h } = setup(); h.onclick({ detail: 1 }); h.onclick({ detail: 2 }); h.ondblclick();
    expect(log).toEqual(['select:p1', 'open:p1']);
  });
  it('Enter opens (and cancels the synthesized click), other keys do not, repeat is ignored', () => {
    const { log, h } = setup(); let prevented = 0;
    h.onkeydown({ key: 'Enter', preventDefault: () => prevented++ });
    h.onkeydown({ key: ' ', preventDefault: () => prevented++ });
    h.onkeydown({ key: 'Enter', repeat: true, preventDefault: () => prevented++ });
    expect(log).toEqual(['open:p1']); expect(prevented).toBe(2);
  });
  it('keyboard-synthesized click (detail 0) selects', () => { const { log, h } = setup(); h.onclick({ detail: 0 }); expect(log).toEqual(['select:p1']); });
});

import { railTileHandlers } from './instanceCard';
describe('rail tile', () => {
  it('one click selects and opens; middle click does nothing', () => {
    const log: string[] = [];
    const h = railTileHandlers('p1', { select: (id) => log.push(`select:${id}`), open: (id) => log.push(`open:${id}`), isSelected: () => false });
    h.onclick({ button: 1 }); expect(log).toEqual([]);
    h.onclick({ button: 0 }); expect(log).toEqual(['select:p1', 'open:p1']);
  });
});

describe('Library cards use one-click open (Home cards keep select / double-click open)', () => {
  it('Library.svelte wires the one-click handlers, Home.svelte the select-then-open ones', async () => {
    const { readFileSync } = await import('node:fs');
    const lib = readFileSync(new URL('../components/Library.svelte', import.meta.url), 'utf8');
    const home = readFileSync(new URL('../components/Home.svelte', import.meta.url), 'utf8');
    expect(lib).toContain('railTileHandlers(id, ops)');
    expect(lib).not.toContain('cardHandlers(');
    expect(home).toContain('cardHandlers(o.id, ops)');
    expect(home).not.toMatch(/go=\{\{/); // instance cards have no play badge
  });
});

describe('play badge and join routing', () => {
  it('only server cards carry the badge; the Server tab always opens the chooser', async () => {
    const { readFileSync } = await import('node:fs');
    const rd = (f: string) => readFileSync(new URL(`../${f}`, import.meta.url), 'utf8');
    expect(rd('components/QuickJoin.svelte')).toContain('<MediaCard title={s.name} playBadge');
    expect(rd('components/Home.svelte')).not.toContain('playBadge');
    const sv = rd('components/Servers.svelte');
    expect(sv).not.toMatch(/requestJoin\((s|x)\)/);
    expect(sv.match(/chooser: true/g)?.length).toBe(2);
    expect(rd('lib/stores.svelte.ts')).toMatch(/if \(opts\.chooser\) \{ servers\.join = s; return; \}/);
  });
});
