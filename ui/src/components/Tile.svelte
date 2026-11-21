<script lang="ts">
  import { fallbackImage, fallbackIndex, type FallbackKind } from '../lib/fallbackImage';
  import { showsFallback, type ImgPhase } from '../lib/imgLoad';
  import Img from './Img.svelte';

  interface Props {
    kind: FallbackKind;
    id: string;
    /** Real image (cover, icon, favicon, account headUrl). Falls back to the MangoClient icon, free-standing (no surface). */
    src?: string | null;
    size?: number;
    alt?: string;
    radius?: number;
    /** Notched corner size in px (signature shape). Overrides radius. */
    notch?: number;
    /** Optional accent (profile colour) mixed into the fallback surface. CSS only, the icon is never recoloured. */
    tint?: string | null;
    /** Stretch to the parent box (cards with a cover area). */
    fill?: boolean;
  }
  let { kind, id, src = null, size = 40, alt = '', radius = 6, notch = 0, tint = null, fill = false }: Props = $props();

  let phase = $state<ImgPhase>('loading');
  const isFb = $derived(showsFallback(phase));
  const tone = $derived(fallbackIndex(kind, id));
</script>

<span
  class="tile" class:fb={isFb} class:notch={notch > 0} data-tone={tone}
  style:width={fill ? '100%' : `${size}px`} style:height={fill ? '100%' : `${size}px`} style:--n="{notch}px"
  style:--tint={tint}
>
  <Img {src} {alt} bind:phase fallback={fallbackImage(kind, id, size * 2)} />
</span>

<style>
  .tile { display: block; flex: none; position: relative; overflow: hidden; background: var(--ink-3); }
  /* the MangoClient icon stands free, with no surface around it (like the skin figures) */
  .tile.fb { background: transparent; }
</style>
