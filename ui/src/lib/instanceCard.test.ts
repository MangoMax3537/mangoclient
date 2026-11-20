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
