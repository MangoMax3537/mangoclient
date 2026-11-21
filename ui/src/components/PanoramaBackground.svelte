<script lang="ts">
  // Hero background of an instance: resolves the instance's background setting (docs/ipc.md Part 7, UI rules in lib/bgCycle.ts) and draws it.
  import { backgroundStyle, panoramaAngle, resolveBackground } from '../lib/instanceIcon';
  import { wallpapers } from '../lib/wallpapers';
  import { loadPanorama, panoramas } from '../lib/stores.svelte';
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
  // only an explicit panorama background needs the extracted panorama (auto uses bundled wallpapers only)
  const wantPano = $derived(background.kind === 'panorama');
  let hidden = $state(typeof document !== 'undefined' && document.hidden);
  $effect(() => { if (wantPano) void loadPanorama(profileId); });

  const res = $derived(resolveBackground(background, { mcVersion, profileId, panorama: panoramas[profileId], wallpapers }));
  const look = $derived.by(() => {
    const d = backgroundStyle(background, res);
    return { dim: dim ?? d.dim, blur: blur ?? d.blur };
  });
</script>

<svelte:document onvisibilitychange={() => (hidden = document.hidden)} />

<BgLayer {res} {look} {tint} angle={panoramaAngle(profileId)} paused={hidden} />
