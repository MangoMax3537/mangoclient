<script lang="ts">
  // Body of a plan / picker dialog: framed list of rows (icon + two bars + an optional trailing box), same frame and row padding as the real list.
  import ListRowSkeleton from './ListRowSkeleton.svelte';
  let { rows = 4, img = 44, notch = 8, tail = 0, framed = true }: { rows?: number; img?: number; notch?: number; tail?: number; framed?: boolean } = $props();
</script>

<div class:fr={framed}>
  <ListRowSkeleton count={rows} {img} {notch} {tail} padding="8px 12px" gap={12} lastBorder={!framed} bars={[{ w: '36%', h: 15 }, { w: '24%', h: 11 }]} />
</div>

<style>
  .fr { border: 1px solid var(--line); border-radius: var(--r-m); overflow: hidden; background: var(--ink-2); }
</style>
