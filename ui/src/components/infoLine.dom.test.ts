// @vitest-environment jsdom
import { describe, expect, it } from 'vitest';
import { flushSync, mount, unmount } from 'svelte';
import InfoLine from './InfoLine.svelte';
import type { Fact } from '../lib/infoLine';

describe('InfoLine', () => {
  it('one aria-hidden icon per fact, an accessible label and a title, no slash separators', () => {
    const facts: Fact[] = [{ kind: 'version', text: '1.21.11' }, { kind: 'loader', text: 'fabric' }, { kind: 'played', text: 'Last played: Nov 16', dim: true }, { kind: 'players', text: '0/500' }, { kind: 'ping', text: '10 ms' }];
    const target = document.createElement('div'); document.body.append(target);
    const a = mount(InfoLine, { target, props: { facts } });
    flushSync();
    const items = [...target.querySelectorAll('[data-fact]')];
    expect(items.map((i) => i.getAttribute('data-fact'))).toEqual(['version', 'loader', 'played', 'players', 'ping']);
    for (const i of items) {
      expect(i.querySelectorAll('svg').length).toBe(1);
      expect(i.querySelector('svg')?.getAttribute('aria-hidden')).toBe('true');
      expect(i.getAttribute('title')).toBeTruthy();
      expect(i.querySelector('.sr-only')?.textContent?.trim()).toBeTruthy();
    }
    expect(target.textContent).not.toContain('/ ');
    expect(items[2].classList.contains('dim')).toBe(true);
    unmount(a);
  });
});
