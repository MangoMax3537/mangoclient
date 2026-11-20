<script lang="ts">
  // A titled group of settings. The hairline above it is drawn by `.set-sections > * + *` (app.css), never by the section itself.
  import type { Snippet } from 'svelte';
  interface Props { id: string; title: string; danger?: boolean; children: Snippet }
  let { id, title, danger = false, children }: Props = $props();
</script>

<section class="set-sec" class:danger aria-labelledby="sec-{id}">
  <h2 id="sec-{id}" class="eyebrow">{title}</h2>
  <div class="set-rows">{@render children()}</div>
</section>

<style>
  .set-sec { display: grid; }
  h2 { padding: 14px 0 4px; }
  .danger h2 { color: var(--tomato); }
</style>
