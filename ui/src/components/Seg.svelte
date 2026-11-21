<script lang="ts">
  interface Opt { value: string; label: string; count?: number; /** Phosphor icon shown before the label (bold, 15px). */ icon?: Component<any>; /** Not selectable: stays visible, dark, with this text as tooltip and accessible description. */ disabledReason?: string }
  interface Props { options: Opt[]; value: string; label: string; onchange: (v: string) => void; role?: 'radiogroup' | 'tablist'; idPrefix?: string; /** Fill the full width: every option the same share, label centered. */ fill?: boolean }
  import type { Component } from 'svelte';
  import { tip } from '../lib/tooltip';
  let { options, value, label, onchange, role = 'radiogroup', idPrefix = 'seg', fill = false }: Props = $props();

  function key(e: KeyboardEvent, i: number) {
    const d = e.key === 'ArrowRight' || e.key === 'ArrowDown' ? 1 : e.key === 'ArrowLeft' || e.key === 'ArrowUp' ? -1 : 0;
    if (!d) return;
    e.preventDefault();
    let n = i;
    for (let k = 0; k < options.length; k++) { n = (n + d + options.length) % options.length; if (!options[n].disabledReason) break; }
    if (options[n].disabledReason) return;
    onchange(options[n].value);
    queueMicrotask(() => document.getElementById(`${idPrefix}-${options[n].value}`)?.focus());
  }
</script>

<div class="segw nb" class:fill>
<div class="seg" {role} aria-label={label}>
  {#each options as o, i (o.value)}
    {@const on = o.value === value}
    <button
      id="{idPrefix}-{o.value}" type="button" class:on class:off={!!o.disabledReason} tabindex={on ? 0 : -1} aria-disabled={o.disabledReason ? 'true' : undefined} use:tip={o.disabledReason ? { text: o.disabledReason, side: 'top' } : null}
      role={role === 'tablist' ? 'tab' : 'radio'} aria-selected={role === 'tablist' ? on : undefined} aria-checked={role === 'radiogroup' ? on : undefined}
      onclick={() => { if (!o.disabledReason) onchange(o.value); }} onkeydown={(e) => key(e, i)}
    >{#if o.icon}<o.icon weight="bold" size={15} aria-hidden="true" />{/if}{o.label}{#if o.disabledReason}<span class="sr-only">: {o.disabledReason}</span>{/if}{#if o.count !== undefined}<span class="c mono">{o.count}</span>{/if}</button>
  {/each}
</div>
</div>

<style>
  /* notched hairline frame (the shared .nb mechanism in app.css: --n cut, 1px --line border, --fill inside); the scroller sits inside it */
  .segw { --n: var(--notch-s); --fill: var(--ink-1); display: inline-flex; max-width: 100%; padding: 1px; vertical-align: top; }
  .segw.fill { display: flex; width: 100%; }
  .seg { display: flex; flex: 1 1 auto; min-width: 0; overflow-x: auto; overflow-y: hidden; scrollbar-width: none; }
  .seg::-webkit-scrollbar { display: none; }
  .segw.fill button { flex: 1 1 0; min-width: max-content; justify-content: center; text-align: center; }
  button { position: relative; height: 34px; padding: 0 14px; color: var(--paper-dim); font-weight: 500; white-space: nowrap; display: inline-flex; align-items: center; gap: 6px; border-right: 1px solid var(--line-soft); transition: color var(--dur) var(--ease), background var(--dur) var(--ease); }
  button:last-child { border-right: 0; }
  /* page tabs: tighter, so the nine instance tabs fit a 960px window without scrolling */
  .seg[role='tablist'] button { padding: 0 10px; }
  button:hover { color: var(--paper); background: var(--ink-3); }
  button.off { color: var(--paper-mute); opacity: 0.55; background: repeating-linear-gradient(135deg, transparent 0 6px, color-mix(in srgb, var(--line) 35%, transparent) 6px 7px); }
  button.off:hover { color: var(--paper-mute); background: repeating-linear-gradient(135deg, transparent 0 6px, color-mix(in srgb, var(--line) 35%, transparent) 6px 7px); }
  button.on { background: var(--ink-4); color: var(--paper); }
  button.on::after { content: ''; position: absolute; left: 8px; right: 8px; bottom: 0; height: 2px; background: var(--accent-fill); }
  /* clip-path cuts outlines, so the ring is drawn inside the button, far enough from the notched corners to stay whole */
  button:focus-visible { outline: 2px solid var(--focus); outline-offset: -4px; }
  .c { font-size: var(--fs-xs); color: var(--paper-mute); }
  button.on .c { color: var(--paper-dim); }
</style>
