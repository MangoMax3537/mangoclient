<script lang="ts">
  // The one horizontal card row of Home ("Weiter spielen" instances, "Schnellbeitritt" servers): same card size, gap and scroll behaviour.
  import type { Snippet } from 'svelte';
  let { children, label = '' }: { children: Snippet; label?: string } = $props();
</script>

<ul class="strip" aria-label={label || undefined}>{@render children()}</ul>

<style>
  .strip { list-style: none; margin: 0 -32px 0 -4px; padding: 4px 32px 8px 4px; display: flex; gap: var(--s-3); overflow-x: auto; scroll-snap-type: x proximity; scrollbar-width: thin; }
</style>
