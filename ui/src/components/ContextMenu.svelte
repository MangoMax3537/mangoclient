<script lang="ts">
  // The context menu, mounted once in App. Opens via lib/contextMenu.svelte.ts (right-click at the pointer, or below a three-dots button).
  // role=menu / menuitem, arrow keys, Home / End, type-ahead, Enter / Space, Esc and Tab close, focus returns to the opener. Closes on outside
  // pointer, scroll, resize and window blur. Disabled items stay visible with their reason as tooltip.
  import { tick } from 'svelte';
  import { cm, closeMenu, edgeIndex, placeMenu, stepIndex, typeaheadIndex } from '../lib/contextMenu.svelte';
  import { tip } from '../lib/tooltip';

  let el: HTMLDivElement | undefined = $state();
  let pos = $state({ left: 0, top: 0 });
  let placed = $state(false);
  let active = $state(-1);
  let buf = '';
  let bufTimer: ReturnType<typeof setTimeout> | undefined;

  $effect(() => {
    void cm.seq;
    if (!cm.open) { placed = false; return; }
    placed = false; active = -1;
    void tick().then(() => {
      if (!el || !cm.open) return;
      const p = placeMenu(cm.at, { w: el.offsetWidth, h: el.offsetHeight }, { w: window.innerWidth, h: window.innerHeight });
      pos = { left: p.left, top: p.top }; placed = true;
      if (cm.keyboard) setActive(edgeIndex(cm.items, 'first'));
      else el.focus({ preventScroll: true });
    });
  });

  function setActive(i: number) {
    active = i;
    if (i >= 0) el?.querySelector<HTMLElement>(`[data-i="${i}"]`)?.focus({ preventScroll: true });
  }
  function activate(i: number) {
    const it = cm.items[i];
    if (!it || it.separator || it.disabled || !it.run) return;
    const run = it.run;
    closeMenu();
    queueMicrotask(run);
  }
  function key(e: KeyboardEvent) {
    const k = e.key;
    if (k === 'Escape') { e.preventDefault(); e.stopPropagation(); closeMenu(); return; }
    if (k === 'Tab') { closeMenu(); return; }
    if (k === 'ArrowDown') setActive(stepIndex(cm.items, active, 1));
    else if (k === 'ArrowUp') setActive(stepIndex(cm.items, active, -1));
    else if (k === 'Home') setActive(edgeIndex(cm.items, 'first'));
    else if (k === 'End') setActive(edgeIndex(cm.items, 'last'));
    else if (k === 'Enter' || k === ' ') { if (active >= 0) activate(active); }
    else if (k.length === 1 && !e.ctrlKey && !e.metaKey && !e.altKey) {
      clearTimeout(bufTimer); buf += k; bufTimer = setTimeout(() => (buf = ''), 700);
      const i = typeaheadIndex(cm.items, buf, buf.length > 1 ? active - 1 : active);
      if (i >= 0) setActive(i);
      e.preventDefault(); return;
    } else return;
    e.preventDefault(); e.stopPropagation();
  }

  // outside interaction closes the menu
  $effect(() => {
    if (!cm.open) return;
    const down = (e: Event) => { if (el && !el.contains(e.target as Node)) closeMenu(false); };
    const scroll = (e: Event) => { if (!(el && e.target instanceof Node && el.contains(e.target))) closeMenu(false); };
    const off = () => closeMenu(false);
    document.addEventListener('pointerdown', down, true);
    document.addEventListener('scroll', scroll, true);
    window.addEventListener('blur', off);
    window.addEventListener('resize', off);
    return () => {
      document.removeEventListener('pointerdown', down, true); document.removeEventListener('scroll', scroll, true);
      window.removeEventListener('blur', off); window.removeEventListener('resize', off);
    };
  });
</script>

{#if cm.open}
  <div class="float" style:left="{pos.left}px" style:top="{pos.top}px" style:visibility={placed ? 'visible' : 'hidden'}>
    <div bind:this={el} class="ctx nb" style:--n="8px" role="menu" tabindex="-1" aria-label={cm.label || undefined} onkeydown={key}>
      {#each cm.items as it, i (it.id + i)}
        {#if it.separator}
          <div class="sep" role="separator"></div>
        {:else}
          <button type="button" role="menuitem" data-i={i} tabindex="-1" class:danger={it.danger} class:cur={active === i} aria-disabled={it.disabled ? 'true' : undefined}
            use:tip={it.disabled && it.reason ? { text: it.reason, side: 'top' } : null}
            onmouseenter={() => setActive(i)} onclick={() => activate(i)}>
            <span class="ic" aria-hidden="true">{#if it.icon}<it.icon weight="bold" size={16} />{/if}</span>
            <span class="lb">{it.label}</span>
            {#if it.shortcut}<kbd class="mono">{it.shortcut}</kbd>{/if}
          </button>
        {/if}
      {/each}
    </div>
  </div>
{/if}

<style>
  /* below tooltips (400) and resize zones (1000), above everything else including the rail */
  .float { position: fixed; z-index: 380; }
  .ctx { min-width: 200px; max-width: 320px; padding: 5px; display: grid; outline: none; animation: ctx-in var(--dur-fast) var(--ease); }
  button { display: flex; align-items: center; gap: 10px; width: 100%; height: 32px; padding: 0 10px 0 8px; text-align: left; color: var(--paper); font-size: var(--fs-md); white-space: nowrap; }
  button:hover:not([aria-disabled='true']), button.cur:not([aria-disabled='true']) { background: var(--ink-4); }
  button:focus-visible { outline: 2px solid var(--focus); outline-offset: -2px; }
  button.danger { color: var(--tomato); }
  button[aria-disabled='true'] { color: var(--paper-mute); opacity: 0.7; }
  .ic { display: grid; place-items: center; width: 16px; flex: none; color: var(--paper-dim); }
  button.danger .ic { color: var(--tomato); }
  .lb { flex: 1; overflow: hidden; text-overflow: ellipsis; }
  kbd { margin-left: 18px; font-size: var(--fs-xs); color: var(--paper-mute); }
  .sep { height: 1px; margin: 4px 4px; background: var(--line); }
  @keyframes ctx-in { from { opacity: 0; transform: translateY(3px); } }
</style>
