<script lang="ts">
  // One cape thumbnail: the outer face drawn from the cape image at integer scale. Loads lazily (`start`), shows its own grey skeleton until the
  // image is drawn, and an error mark (never a spinner that runs forever) when the image fails or takes longer than the load timeout.
  import WarningCircleIcon from 'phosphor-svelte/lib/WarningCircleIcon';
  import { drawCapeFront } from '../lib/skinFlat';
  import { LOAD_TIMEOUT_MS } from '../lib/loadState.svelte';
  import { t } from '../lib/stores.svelte';

  let { url, start = true }: { url: string; start?: boolean } = $props();
  let phase = $state<'idle' | 'loading' | 'ready' | 'error'>('idle');
  let canvas: HTMLCanvasElement | undefined = $state();

  $effect(() => {
    const u = url;
    if (!start || !canvas) return;
    phase = 'loading';
    const img = new Image();
    const timer = setTimeout(() => { img.src = ''; if (phase === 'loading') phase = 'error'; }, LOAD_TIMEOUT_MS);
    img.onload = () => {
      clearTimeout(timer);
      const c = canvas?.getContext('2d');
      if (c && canvas) { drawCapeFront(c, img, 6); canvas.dataset.ready = '1'; phase = 'ready'; } else phase = 'error';
    };
    img.onerror = () => { clearTimeout(timer); phase = 'error'; };
    img.src = u;
    return () => { clearTimeout(timer); img.onload = null; img.onerror = null; };
  });
</script>

<span class="ct" class:ready={phase === 'ready'}>
  <canvas bind:this={canvas} class="px" width="60" height="96" aria-hidden="true"></canvas>
  {#if phase === 'idle' || phase === 'loading'}<span class="skel sk" aria-hidden="true"></span>{/if}
  {#if phase === 'error'}<span class="bad" role="img" aria-label={t('load.failed')}><WarningCircleIcon weight="bold" size={26} /></span>{/if}
</span>

<style>
  .ct { position: relative; display: block; width: 60px; height: 96px; }
  canvas { display: block; width: 60px; height: 96px; opacity: 0; transition: opacity var(--dur) var(--ease); }
  .ready canvas { opacity: 1; }
  .sk { position: absolute; inset: 0; }
  .bad { position: absolute; inset: 0; display: grid; place-items: center; color: var(--paper-mute); }
</style>
