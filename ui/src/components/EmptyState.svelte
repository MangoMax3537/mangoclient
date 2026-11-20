<script lang="ts">
  import type { Snippet } from 'svelte';
  import { brandIcon } from '../lib/fallbackImage';

  let { title, hint = '', children }: { title: string; hint?: string; children?: Snippet } = $props();
</script>

<div class="empty">
  <span class="img notch" style:--n="18px"><img class="px" src={brandIcon(256)} alt="" draggable="false" /></span>
  <div class="txt">
    <h2>{title}</h2>
    {#if hint}<p class="hint">{hint}</p>{/if}
    {#if children}<div class="cta">{@render children()}</div>{/if}
  </div>
</div>

<style>
  .empty { display: flex; flex-wrap: wrap; align-items: center; gap: var(--s-5); padding: var(--s-6) var(--s-2); }
  .img { width: 132px; height: 132px; flex: none; display: grid; place-items: center; background: var(--ink-3); }
  .img img { width: 78%; height: 78%; object-fit: contain; opacity: 0.9; }
  .txt { display: grid; gap: var(--s-2); justify-items: start; flex: 1 1 240px; min-width: 0; }
  h2 { font-size: var(--fs-2xl); line-height: 1.05; overflow-wrap: anywhere; }
  p { margin: 0; max-width: 40ch; }
  .cta { margin-top: var(--s-2); display: flex; flex-wrap: wrap; gap: var(--s-2); }
</style>
