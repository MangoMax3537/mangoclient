<script lang="ts">
  import { fallbackImage, type FallbackKind } from '../lib/fallbackImage';

  interface Props {
    kind: FallbackKind;
    id: string;
    /** Real image (cover, icon, favicon, account headUrl). Falls back to the bundled tile on error. */
    src?: string | null;
    size?: number;
    alt?: string;
    radius?: number;
  }
  let { kind, id, src = null, size = 40, alt = '', radius = 6 }: Props = $props();

  let failed = $state(false);
  const fb = $derived(fallbackImage(kind, id));
  const url = $derived(failed ? fb : src || fb);
  $effect(() => { void src; void id; failed = false; });
</script>

<img
  class="tile px"
  class:real={!!src && !failed && kind !== 'account'}
  src={url}
  {alt}
  width={size}
  height={size}
  style:border-radius="{radius}px"
  draggable="false"
  onerror={() => (failed = true)}
/>

<style>
  .tile { display: block; flex: none; object-fit: cover; background: var(--ink-3); }
  .tile.real { image-rendering: auto; }
</style>
