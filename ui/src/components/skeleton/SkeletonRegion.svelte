<script lang="ts">
  // Root of every skeleton layout: a polite live region with a visually hidden "Loading…" (announced once, when the skeleton appears),
  // the shapes themselves are aria-hidden. `class` carries the layout of the final content so nothing shifts when it arrives.
  import type { Snippet } from 'svelte';
  import { t } from '../../lib/stores.svelte';

  let { class: cls = '', style = '', announce = true, children }: { class?: string; style?: string; announce?: boolean; children: Snippet } = $props();
</script>

<div class={cls} {style} role={announce ? 'status' : undefined} aria-busy="true">
  {#if announce}<span class="sr-only">{t('skel.loading')}</span>{/if}
  <div class="skr" aria-hidden="true">{@render children()}</div>
</div>

<style>
  .skr { display: contents; }
</style>
