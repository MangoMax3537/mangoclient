<script lang="ts">
  // A skin drawn flat on a canvas at integer scale, nearest-neighbour: the library previews, the dialogs' previews. Plain <img> loading is
  // enough here (a canvas may draw cross-origin images, it just cannot read them back).
  import { drawFlat, FIGURE } from '../lib/skinFlat';

  interface Props { url: string; slim: boolean; side?: 'front' | 'back'; scale?: number; cape?: string | null; label?: string; fill?: boolean }
  let { url, slim, side = 'front', scale = 4, cape = null, label = '', fill = false }: Props = $props();
  let cv: HTMLCanvasElement | undefined = $state();
  let loaded = $state(false);
  let failed = $state(false);
  let skinImg = $state.raw<HTMLImageElement | null>(null);
  let capeImg = $state.raw<HTMLImageElement | null>(null);

  const load = (src: string, ok: (i: HTMLImageElement) => void, bad: () => void) => {
    const i = new Image();
    i.onload = () => ok(i); i.onerror = bad;
    i.src = src;
    return i;
  };
  $effect(() => { const u = url; loaded = false; failed = false; skinImg = null; load(u, (i) => { if (u === url) { skinImg = i; } }, () => { if (u === url) failed = true; }); });
  $effect(() => { const u = cape; capeImg = null; if (u) load(u, (i) => { if (u === cape) capeImg = i; }, () => {}); });
  $effect(() => {
    if (!cv || !skinImg) return;
    const ctx = cv.getContext('2d');
    if (!ctx) return;
    drawFlat(ctx, skinImg, { slim, side, scale, cape: capeImg });
    loaded = true;
  });
</script>

<span class="flat" class:fill style:aspect-ratio="{FIGURE.w} / {FIGURE.h}" role="img" aria-label={label}>
  {#if !loaded && !failed}<span class="skel" aria-hidden="true"></span>{/if}
  <canvas bind:this={cv} class="px" class:ready={loaded} width={FIGURE.w * scale} height={FIGURE.h * scale} aria-hidden="true"></canvas>
</span>

<style>
  .flat { position: relative; display: block; height: 100%; max-width: 100%; }
  .flat.fill { width: 100%; height: auto; }
  canvas { display: block; width: 100%; height: 100%; opacity: 0; transition: opacity var(--dur) var(--ease); }
  canvas.ready { opacity: 1; }
  .skel { position: absolute; inset: 0; border-radius: var(--r-s); }
</style>
