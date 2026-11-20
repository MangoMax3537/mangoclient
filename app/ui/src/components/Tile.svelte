<script lang="ts">
  import { fallbackImage, accountHeadUrl, type FallbackKind } from '../lib/fallbackImage';

  interface Props {
    kind: FallbackKind;
    id: string;
    /** Real image (cover, icon, favicon). Falls back to the bundled tile on error. */
    src?: string | null;
    /** For accounts: pass type so Microsoft accounts try the real skin head. */
    accountType?: 'microsoft' | 'offline';
    size?: number;
    alt?: string;
    radius?: number;
  }
  let { kind, id, src = null, accountType, size = 40, alt = '', radius = 6 }: Props = $props();

  let failed = $state(false);
  const fb = $derived(fallbackImage(kind, id));
  const url = $derived(
    failed ? fb
      : src ? src
      : kind === 'account' && accountType === 'microsoft' ? accountHeadUrl({ uuid: id, type: 'microsoft' })
      : fb,
  );
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
