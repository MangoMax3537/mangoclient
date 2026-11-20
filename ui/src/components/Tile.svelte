<script lang="ts">
  import { fallbackImage, fallbackIndex, type FallbackKind } from '../lib/fallbackImage';

  interface Props {
    kind: FallbackKind;
    id: string;
    /** Real image (cover, icon, favicon, account headUrl). Falls back to the MangoClient icon on a neutral surface. */
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

  let failed = $state(false);
  const isFb = $derived(failed || !src);
  const url = $derived(isFb ? fallbackImage(kind, id, size * 2) : (src as string));
  const tone = $derived(fallbackIndex(kind, id));
  $effect(() => { void src; void id; failed = false; });
</script>

<span
  class="tile" class:fb={isFb} class:notch={notch > 0} data-tone={tone}
  style:width={fill ? '100%' : `${size}px`} style:height={fill ? '100%' : `${size}px`} style:--n="{notch}px"
  style:border-radius={notch ? undefined : `${radius}px`}
  style:--tint={tint}
>
  <img class:px={isFb} src={url} {alt} draggable="false" onerror={() => (failed = true)} />
</span>

<style>
  .tile { display: block; flex: none; position: relative; overflow: hidden; background: var(--ink-3); }
  .tile img { display: block; width: 100%; height: 100%; object-fit: cover; }
  .tile.fb { background: var(--tile-bg, var(--ink-3)); }
  .tile.fb[data-tone='0'] { --tile-bg: var(--ink-3); }
  .tile.fb[data-tone='1'] { --tile-bg: var(--ink-4); }
  .tile.fb[data-tone='2'] { --tile-bg: color-mix(in srgb, var(--ink-3) 82%, var(--leaf)); }
  .tile.fb[data-tone='3'] { --tile-bg: color-mix(in srgb, var(--ink-3) 86%, var(--mango)); }
  .tile.fb[style*='--tint'] { background: color-mix(in srgb, var(--ink-3) 78%, var(--tint)); }
  .tile.fb img { object-fit: contain; padding: 12%; }
</style>
