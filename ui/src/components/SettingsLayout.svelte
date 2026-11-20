<script lang="ts">
  // Centred settings column (max 800 px) with the "on this page" navigator in a reserved column on the right (>= 1100 px wide windows).
  // The right column is reserved even when the navigator is not rendered, so the content never shifts (search results, skeleton).
  import type { Snippet } from 'svelte';
  import SettingsNav from './SettingsNav.svelte';
  import { showNavigator } from '../lib/scrollspy';
  interface Props { items: { id: string; title: string }[]; width?: number; children: Snippet }
  let { items, width = 800, children }: Props = $props();
</script>

<div class="lay" style:--w="{width}px">
  <div class="col">{@render children()}</div>
  <div class="side">{#if showNavigator(items)}<SettingsNav {items} />{/if}</div>
</div>

<style>
  .lay { display: grid; grid-template-columns: minmax(0, 1fr) minmax(0, var(--w)) minmax(0, 1fr); padding: 0 32px; }
  .col { grid-column: 2; min-width: 0; }
  .side { display: none; }
  @media (min-width: 1100px) {
    .lay { grid-template-columns: minmax(0, 1fr) minmax(0, var(--w)) minmax(176px, 1fr); }
    .side { display: block; grid-column: 3; align-self: start; position: sticky; top: 28px; padding-left: 28px; max-width: 240px; }
  }
</style>
