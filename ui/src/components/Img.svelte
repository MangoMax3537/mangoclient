<script lang="ts">
  // The one image slot. Until the `load` event it is a plain grey pulsing box (no icon, no text); then the image fades in (opacity only).
  // On `error`, or when there is no URL, it shows the real fallback image (MangoClient icon via fallbackImage) instead. The slot always
  // fills its parent (or takes `aspect`), so nothing shifts when the image arrives.
  import { imgPhase, imgWatch, showsFallback, type ImgPhase } from '../lib/imgLoad';

  interface Props {
    src?: string | null;
    alt?: string;
    /** Fallback image URL (from fallbackImage); null = leave the slot empty-grey on failure. */
    fallback?: string | null;
    fit?: 'cover' | 'contain' | 'fill';
    /** Pixel art: nearest-neighbour scaling. */
    pixel?: boolean;
    /** Padding of the fallback image inside the slot. */
    fbPad?: string;
    /** CSS aspect-ratio, e.g. `16 / 10`. Without it the slot is as tall as its parent. */
    aspect?: string;
    width?: number;
    height?: number;
    lazy?: boolean;
    class?: string;
    phase?: ImgPhase;
    onfail?: () => void;
  }
  let {
    src = null, alt = '', fallback = null, fit = 'cover', pixel = false, fbPad = '12%', aspect, width, height, lazy = false, class: cls = '',
    phase = $bindable('loading'), onfail,
  }: Props = $props();

  let loaded = $state(false);
  let failed = $state(false);
  let instant = $state(false);
  let seen: string | null | undefined;
  $effect.pre(() => { if (src !== seen) { seen = src; loaded = false; failed = false; instant = false; } });
  const ph = $derived(imgPhase(src, loaded, failed));
  $effect(() => { phase = ph; });
  $effect(() => { if (ph === 'error') onfail?.(); });
  const watch = $derived({
    src,
    onload: (i: boolean) => { instant = i; loaded = true; },
    onerror: () => { failed = true; },
  });
</script>

<span class="im {cls}" style:aspect-ratio={aspect} style:height={aspect ? 'auto' : undefined} data-phase={ph}>
  {#if ph === 'loading'}<span class="skel fill" aria-hidden="true"></span>{/if}
  {#if src && !failed}
    <img
      class:px={pixel} class:ready={loaded} class:instant {src} {alt} {width} {height} loading={lazy ? 'lazy' : undefined} decoding="async" draggable="false"
      style:object-fit={fit} use:imgWatch={watch}
    />
  {/if}
  {#if showsFallback(ph) && fallback}
    <img class="fb px" src={fallback} {alt} draggable="false" style:padding={fbPad} />
  {/if}
</span>

<style>
  .im { display: block; position: relative; width: 100%; height: 100%; overflow: hidden; }
  .fill { position: absolute; inset: 0; display: block; }
  img { position: absolute; inset: 0; width: 100%; height: 100%; display: block; opacity: 0; transition: opacity var(--dur) var(--ease); }
  img.ready { opacity: 1; }
  img.instant { transition: none; }
  img.fb { opacity: 1; object-fit: contain; transition: none; }
</style>
