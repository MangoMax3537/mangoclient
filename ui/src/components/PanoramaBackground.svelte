<script lang="ts">
  // Hero background of an instance: resolves the instance's background setting (docs/ipc.md Part 7, UI rules in lib/bgCycle.ts) and draws it.
  import { backgroundStyle, panoramaAngle, resolveBackground } from '../lib/instanceIcon';
  import { buildPool } from '../lib/bgCycle';
  import { wallpapers } from '../lib/wallpapers';
  import { loadPanorama, panoramaPool, panoramas } from '../lib/stores.svelte';
  import type { ProfileBackground } from '../lib/types';
  import BgLayer from './BgLayer.svelte';

  interface Props {
    profileId: string;
    mcVersion: string;
    background: ProfileBackground;
    /** Instance colour for the accent bar and the glow of the plain surface. */
    tint?: string | null;
    /** Overrides the background's own dim / blur (the picker's live preview while a slider moves). */
    dim?: number | null;
    blur?: number | null;
  }
  let { profileId, mcVersion, background, tint = null, dim = null, blur = null }: Props = $props();
  // `auto` also needs the instance's own panorama (step b), an explicit panorama obviously too
  const wantPano = $derived(background.kind === 'panorama' || background.kind === 'auto');
  let hidden = $state(typeof document !== 'undefined' && document.hidden);
  $effect(() => { if (wantPano) void loadPanorama(profileId); });

  const pool = $derived(buildPool(wallpapers, panoramaPool.sets));
  const res = $derived(resolveBackground(background, { mcVersion, profileId, panorama: panoramas[profileId], wallpapers, pool }));
  const look = $derived.by(() => {
    const d = backgroundStyle(background, res);
    return { dim: dim ?? d.dim, blur: blur ?? d.blur };
  });
</script>

<svelte:document onvisibilitychange={() => (hidden = document.hidden)} />

<BgLayer {res} {look} {tint} angle={panoramaAngle(profileId)} paused={hidden} />
