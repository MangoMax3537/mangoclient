<script lang="ts" generics="T">
  import type { Snippet } from 'svelte';

  interface Props {
    items: T[];
    rowHeight: number;
    key: (item: T, index: number) => string | number;
    row: Snippet<[T, number]>;
    label: string;
    overscan?: number;
    /** One hairline between neighbouring rows (never at the list's own edges). */
    separators?: boolean;
  }
  let { items, rowHeight, key, row, label, overscan = 6, separators = false }: Props = $props();
  let top = $state(0);
  let h = $state(600);
  const start = $derived(Math.max(0, Math.floor(top / rowHeight) - overscan));
  const end = $derived(Math.min(items.length, Math.ceil((top + h) / rowHeight) + overscan));
  const slice = $derived(items.slice(start, end));
</script>

<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
<div class="vl" class:sep={separators} role="list" aria-label={label} bind:clientHeight={h} onscroll={(e) => (top = e.currentTarget.scrollTop)} tabindex="-1">
  <div class="space" style:height="{items.length * rowHeight}px">
    <div class="win" style:transform="translateY({start * rowHeight}px)">
      {#each slice as item, i (key(item, start + i))}
        <div class="r" role="listitem" style:height="{rowHeight}px">{@render row(item, start + i)}</div>
      {/each}
    </div>
  </div>
</div>

<style>
  .vl { overflow: auto; height: 100%; min-height: 0; contain: strict; }
  .space { position: relative; }
  .win { position: absolute; left: 0; right: 0; top: 0; will-change: transform; }
  .r { overflow: hidden; }
  /* one hairline between neighbouring rows, never at the list's own edges (the list's container draws those) */
  .sep .r + .r { border-top: 1px solid var(--line-soft); }
</style>
