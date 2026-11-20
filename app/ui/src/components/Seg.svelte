<script lang="ts">
  interface Opt { value: string; label: string; count?: number }
  interface Props { options: Opt[]; value: string; label: string; onchange: (v: string) => void; role?: 'radiogroup' | 'tablist'; idPrefix?: string }
  let { options, value, label, onchange, role = 'radiogroup', idPrefix = 'seg' }: Props = $props();

  function key(e: KeyboardEvent, i: number) {
    const d = e.key === 'ArrowRight' || e.key === 'ArrowDown' ? 1 : e.key === 'ArrowLeft' || e.key === 'ArrowUp' ? -1 : 0;
    if (!d) return;
    e.preventDefault();
    const n = (i + d + options.length) % options.length;
    onchange(options[n].value);
    queueMicrotask(() => document.getElementById(`${idPrefix}-${options[n].value}`)?.focus());
  }
</script>

<div class="seg" {role} aria-label={label}>
  {#each options as o, i (o.value)}
    {@const on = o.value === value}
    <button
      id="{idPrefix}-{o.value}" type="button" class:on tabindex={on ? 0 : -1}
      role={role === 'tablist' ? 'tab' : 'radio'} aria-selected={role === 'tablist' ? on : undefined} aria-checked={role === 'radiogroup' ? on : undefined}
      onclick={() => onchange(o.value)} onkeydown={(e) => key(e, i)}
    >{o.label}{#if o.count !== undefined}<span class="c mono">{o.count}</span>{/if}</button>
  {/each}
</div>

<style>
  .seg { display: inline-flex; border: 1px solid var(--line); border-radius: var(--r-m); overflow: hidden; background: var(--ink-1); max-width: 100%; }
  button { height: 34px; padding: 0 12px; color: var(--paper-dim); font-weight: 500; white-space: nowrap; display: inline-flex; align-items: center; gap: 6px; border-right: 1px solid var(--line-soft); }
  button:last-child { border-right: 0; }
  button:hover { color: var(--paper); background: var(--ink-3); }
  button.on { background: var(--ink-4); color: var(--mango); }
  .c { font-size: 12px; color: var(--paper-mute); }
  button.on .c { color: var(--paper-dim); }
</style>
