<script lang="ts">
  // Same geometry as the first screen of a page (hero block + a row of cards): shown while a screen chunk loads, so the layout does not jump.
  import { t } from '../lib/stores.svelte';
  let { cards = 4 }: { cards?: number } = $props();
</script>

<div class="loading" role="status"><span class="sr-only">{t('common.loading')}</span>
  <div class="skel hero-sk" aria-hidden="true"></div>
  <div class="cards-sk" aria-hidden="true">{#each Array(cards) as _, i (i)}<div class="skel notch" style:--n="14px"></div>{/each}</div>
</div>

<style>
  .loading { height: 100%; display: grid; grid-template-rows: 46vh auto; gap: var(--s-5); align-content: start; }
  .hero-sk { opacity: 0.6; }
  .cards-sk { display: flex; gap: var(--s-3); padding: 0 32px 0 44px; }
  .cards-sk div { width: 212px; height: 200px; }
</style>
