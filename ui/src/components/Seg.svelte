<script lang="ts">
  interface Opt { value: string; label: string; count?: number; /** Not selectable: stays visible, dark, with this text as tooltip and accessible description. */ disabledReason?: string }
  interface Props { options: Opt[]; value: string; label: string; onchange: (v: string) => void; role?: 'radiogroup' | 'tablist'; idPrefix?: string; /** Fill the full width: every option the same share, label centered. */ fill?: boolean }
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

<div class="seg" class:fill {role} aria-label={label}>
  {#each options as o, i (o.value)}
    {@const on = o.value === value}
    <button
      id="{idPrefix}-{o.value}" type="button" class:on class:off={!!o.disabledReason} tabindex={on ? 0 : -1} aria-disabled={o.disabledReason ? 'true' : undefined} use:tip={o.disabledReason ? { text: o.disabledReason, side: 'top' } : null}
      role={role === 'tablist' ? 'tab' : 'radio'} aria-selected={role === 'tablist' ? on : undefined} aria-checked={role === 'radiogroup' ? on : undefined}
      onclick={() => { if (!o.disabledReason) onchange(o.value); }} onkeydown={(e) => key(e, i)}
    >{o.label}{#if o.disabledReason}<span class="sr-only">: {o.disabledReason}</span>{/if}{#if o.count !== undefined}<span class="c mono">{o.count}</span>{/if}</button>
  {/each}
</div>

<style>
  .seg { display: inline-flex; border: 1px solid var(--line); overflow-x: auto; overflow-y: hidden; scrollbar-width: none; background: var(--ink-1); max-width: 100%; }
  .seg::-webkit-scrollbar { display: none; }
  .seg.fill { display: flex; width: 100%; }
  .seg.fill button { flex: 1 1 0; min-width: max-content; justify-content: center; text-align: center; }
  button { position: relative; height: 34px; padding: 0 14px; color: var(--paper-dim); font-weight: 500; white-space: nowrap; display: inline-flex; align-items: center; gap: 6px; border-right: 1px solid var(--line-soft); transition: color var(--dur) var(--ease), background var(--dur) var(--ease); }
  button:last-child { border-right: 0; }
  /* page tabs: tighter, so the nine instance tabs fit a 960px window without scrolling */
  .seg[role='tablist'] button { padding: 0 10px; }
  button:hover { color: var(--paper); background: var(--ink-3); }
  button.off { color: var(--paper-mute); opacity: 0.55; background: repeating-linear-gradient(135deg, transparent 0 6px, color-mix(in srgb, var(--line) 35%, transparent) 6px 7px); }
  button.off:hover { color: var(--paper-mute); background: repeating-linear-gradient(135deg, transparent 0 6px, color-mix(in srgb, var(--line) 35%, transparent) 6px 7px); }
  button.on { background: var(--ink-4); color: var(--paper); }
  button.on::after { content: ''; position: absolute; left: 8px; right: 8px; bottom: 0; height: 2px; background: var(--accent-fill); }
  button:focus-visible { outline-offset: -3px; }
  .c { font-size: var(--fs-xs); color: var(--paper-mute); }
  button.on .c { color: var(--paper-dim); }
</style>
