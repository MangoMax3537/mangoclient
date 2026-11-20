<script lang="ts">
  // A titled group of settings. Sections are separated by whitespace only (`.set-sections` gap, app.css), never by a line.
  import type { Snippet } from 'svelte';
  import { enter } from '../lib/motion';
  interface Props { id: string; title: string; danger?: boolean; children: Snippet }
  let { id, title, danger = false, children }: Props = $props();
</script>

<section class="set-sec" class:danger aria-labelledby="sec-{id}" in:enter={{ y: 6, duration: 160 }}>
  <h2 id="sec-{id}" class="eyebrow" tabindex="-1">{title}</h2>
  <div class="set-rows">{@render children()}</div>
</section>

<style>
  .set-sec { display: grid; }
  h2 { padding: 0 0 6px; }
  h2:focus-visible { outline-offset: 4px; }
  .danger h2 { color: var(--tomato); }
</style>
