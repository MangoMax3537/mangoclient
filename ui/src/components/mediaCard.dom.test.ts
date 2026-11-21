// @vitest-environment jsdom
import { describe, expect, it, vi } from 'vitest';
import { createRawSnippet, flushSync, mount, unmount } from 'svelte';
import MediaCard from './MediaCard.svelte';

const art = createRawSnippet(() => ({ render: () => '<span></span>' }));
const click = (el: Element, detail = 1) => el.dispatchEvent(new MouseEvent('click', { bubbles: true, detail }));

describe('MediaCard click wiring', () => {
  it('a card with `handlers` and no `onclick` keeps the handlers\' click (Home: one click selects the instance)', () => {
    const target = document.body.appendChild(document.createElement('div'));
    const onclick = vi.fn();
    const ondblclick = vi.fn();
    const c = mount(MediaCard, { target, props: { title: 'A', art, handlers: { onclick, ondblclick } } });
    flushSync();
    const btn = target.querySelector('button.open') as HTMLButtonElement;
    click(btn);
    btn.dispatchEvent(new MouseEvent('dblclick', { bubbles: true }));
    expect(onclick).toHaveBeenCalledTimes(1);
    expect(ondblclick).toHaveBeenCalledTimes(1);
    unmount(c); target.remove();
  });
  it('a card with only `onclick` (Home quick join) still gets it', () => {
    const target = document.body.appendChild(document.createElement('div'));
    const onclick = vi.fn();
    const c = mount(MediaCard, { target, props: { title: 'S', art, onclick } });
    flushSync();
    click(target.querySelector('button.open') as HTMLButtonElement);
    expect(onclick).toHaveBeenCalledTimes(1);
    unmount(c); target.remove();
  });
});
