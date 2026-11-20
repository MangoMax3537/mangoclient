<script lang="ts">
  // "On this page": sticky list of the section headings, a thin marker on the active one (scrollspy), click scrolls to the section and moves focus to its heading.
  import { onMount } from 'svelte';
  import { activeSection, type SectionTop } from '../lib/scrollspy';
  import { t } from '../lib/stores.svelte';

  interface Props { items: { id: string; title: string }[] }
  let { items }: Props = $props();

  let nav: HTMLElement | undefined = $state();
  let active = $state<string | null>(null);
  const headId = (id: string) => `sec-${id}`;

  function scroller(el: HTMLElement | null): HTMLElement | null {
    for (let n = el?.parentElement ?? null; n; n = n.parentElement) {
      const o = getComputedStyle(n).overflowY;
      if (o === 'auto' || o === 'scroll') return n;
    }
    return null;
  }
  function measure() {
    const root = scroller(nav ?? null);
    if (!root) return;
    const top = root.getBoundingClientRect().top;
    const tops: SectionTop[] = [];
    for (const it of items) {
      const el = document.getElementById(headId(it.id))?.closest('section');
      if (el) tops.push({ id: it.id, top: el.getBoundingClientRect().top - top });
    }
    active = activeSection(tops, { atBottom: root.scrollHeight - root.clientHeight > 8 && root.scrollTop + root.clientHeight >= root.scrollHeight - 2 });
  }
  let raf = 0;
  const queue = () => { if (!raf) raf = requestAnimationFrame(() => { raf = 0; measure(); }); };
  onMount(() => {
    const root = scroller(nav ?? null);
    root?.addEventListener('scroll', queue, { passive: true });
    window.addEventListener('resize', queue);
    measure();
    return () => { root?.removeEventListener('scroll', queue); window.removeEventListener('resize', queue); cancelAnimationFrame(raf); };
  });
  $effect(() => { void items; queueMicrotask(queue); });

  function go(id: string) {
    const h = document.getElementById(headId(id));
    const sec = h?.closest('section');
    if (!h || !sec) return;
    const calm = globalThis.matchMedia?.('(prefers-reduced-motion: reduce)').matches;
    sec.scrollIntoView({ behavior: calm ? 'auto' : 'smooth', block: 'start' });
    h.focus({ preventScroll: true });
    active = id;
  }
</script>

<nav bind:this={nav} class="sn" aria-label={t('settings.onPage')}>
  <span class="eyebrow cap" aria-hidden="true">{t('settings.onPage')}</span>
  <ul>
    {#each items as it (it.id)}
      <li><button type="button" class="mono" class:on={active === it.id} aria-current={active === it.id ? 'true' : undefined} onclick={() => go(it.id)}>{it.title}</button></li>
    {/each}
  </ul>
</nav>

<style>
  .sn { display: grid; gap: 10px; }
  .cap { font-size: 11px; color: var(--paper-mute); }
  ul { list-style: none; margin: 0; padding: 0; display: grid; border-left: 1px solid var(--line); }
  button { position: relative; display: block; width: 100%; text-align: left; padding: 6px 12px; font-size: 12px; letter-spacing: 0.04em; text-transform: uppercase; color: var(--paper-mute); transition: color var(--dur) var(--ease); }
  button:hover { color: var(--paper); }
  button.on { color: var(--paper); }
  button.on::before { content: ''; position: absolute; left: -2px; top: 4px; bottom: 4px; width: 3px; background: var(--accent-fill); }
  button:focus-visible { outline-offset: -2px; }
</style>
