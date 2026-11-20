<script lang="ts">
  // 3D player model (WebGL, lib/skin3d.ts) with a flat front + back fallback (2D canvas) when WebGL is unavailable or refuses the texture.
  // The GL context only lives while this component is mounted (the Skins page destroys it when you leave) and the loop pauses while the
  // document is hidden. Textures: lib/skinTexture.ts (fetch -> blob -> ImageBitmap first, CORS <img> second, plain <img> = flat only).
  import { onDestroy, onMount } from 'svelte';
  import { SkinRenderer, webglAvailable, type Pose } from '../lib/skin3d';
  import { drawFlat, FIGURE } from '../lib/skinFlat';
  import { glSafe, loadTexture, type LoadedTexture, type TexPath } from '../lib/skinTexture';

  interface Props {
    skinUrl: string;
    capeUrl?: string | null;
    slim: boolean;
    pose?: Pose;
    /** The cape sways (default) or hangs still. */
    capeAnim?: boolean;
    label: string;
    /** `loading` until the skin is on screen; `gl` / `flat` afterwards; `error` when the skin image could not be loaded. */
    mode?: 'loading' | 'gl' | 'flat' | 'error';
    /** Which texture path delivered the skin (diagnostics / the report). */
    path?: TexPath | null;
  }
  let { skinUrl, capeUrl = null, slim, pose = 'idle', capeAnim = true, label, mode = $bindable('loading'), path = $bindable(null) }: Props = $props();

  let glCanvas: HTMLCanvasElement | undefined = $state();
  let front: HTMLCanvasElement | undefined = $state();
  let back: HTMLCanvasElement | undefined = $state();
  let renderer = $state.raw<SkinRenderer | null>(null);
  let useGl = $state(true);
  let skin = $state.raw<LoadedTexture | null>(null);
  let cape = $state.raw<LoadedTexture | null>(null);
  let failed = $state(false);

  onMount(() => {
    useGl = webglAvailable();
    const vis = () => renderer?.setActive(!document.hidden);
    document.addEventListener('visibilitychange', vis);
    return () => document.removeEventListener('visibilitychange', vis);
  });
  onDestroy(() => { renderer?.destroy(); renderer = null; });

  // renderer exists as soon as the canvas does
  $effect(() => {
    if (!useGl || !glCanvas || renderer) return;
    try {
      renderer = new SkinRenderer(glCanvas, { slim });
      renderer.onLost = () => { useGl = false; renderer = null; };
    } catch { useGl = false; }
  });

  let seq = 0;
  $effect(() => {
    const url = skinUrl;
    const my = ++seq;
    failed = false;
    loadTexture(url, { allowPlain: true }).then((t) => { if (my === seq) { skin = t; path = t.path; } }, () => { if (my === seq) failed = true; });
  });
  let cseq = 0;
  $effect(() => {
    const url = capeUrl;
    const my = ++cseq;
    if (!url) { cape = null; return; }
    loadTexture(url, { allowPlain: true }).then((t) => { if (my === cseq) cape = t; }, () => { if (my === cseq) cape = null; });
  });

  // push the state into the renderer; a texture the GL refuses (tainted) switches to the flat view
  $effect(() => {
    const r = renderer, s = skin;
    if (!useGl || !r || !s) return;
    if (!glSafe(s)) { useGl = false; return; }
    try { r.setSkin(s.source); } catch { useGl = false; }
  });
  $effect(() => {
    const r = renderer, c = cape;
    if (!useGl || !r) return;
    if (!c) { r.setCape(null); return; }
    if (!glSafe(c)) { r.setCape(null); return; }
    try { r.setCape(c.source); } catch { r.setCape(null); }
  });
  $effect(() => { renderer?.setSlim(slim); });
  $effect(() => { renderer?.setPose(pose); });
  $effect(() => { renderer?.setCapeAnim(capeAnim); });
  /** Back to the start angle (the toolbar button next to the box). */
  export function resetView(): void { renderer?.resetView(); }
  // leaving GL: free the context
  $effect(() => { if (!useGl && renderer) { renderer.destroy(); renderer = null; } });

  // flat fallback
  $effect(() => {
    if (useGl || !skin || !front || !back) return;
    for (const [cv, side] of [[front, 'front'], [back, 'back']] as const) {
      const ctx = cv.getContext('2d');
      if (ctx) drawFlat(ctx, skin.source, { slim, side, scale: 10, cape: cape?.source ?? null });
    }
  });

  $effect(() => {
    mode = failed ? 'error' : !skin ? 'loading' : useGl ? 'gl' : 'flat';
  });
</script>

<div class="viewer" class:flat={!useGl}>
  {#if useGl}
    <canvas bind:this={glCanvas} class="gl" class:ready={mode === 'gl'} tabindex="0" aria-label={label}></canvas>
  {:else}
    <div class="figures" role="img" aria-label={label}>
      <canvas bind:this={front} class="fig px" width={FIGURE.w * 10} height={FIGURE.h * 10}></canvas>
      <canvas bind:this={back} class="fig px" width={FIGURE.w * 10} height={FIGURE.h * 10}></canvas>
    </div>
  {/if}
  {#if mode === 'loading'}<span class="skel wait" aria-hidden="true"></span>{/if}
</div>

<style>
  .viewer { position: relative; width: 100%; height: 100%; min-height: 0; }
  .gl { display: block; width: 100%; height: 100%; opacity: 0; transition: opacity var(--dur) var(--ease); outline-offset: -4px; }
  .gl.ready { opacity: 1; }
  .wait { position: absolute; left: 50%; top: 8%; bottom: 8%; width: min(34%, 150px); transform: translateX(-50%); }
  .figures { display: flex; gap: 6%; align-items: center; justify-content: center; width: 100%; height: 100%; padding: 4% 5% 14%; }
  .fig { width: 45%; max-height: 100%; height: auto; aspect-ratio: 1 / 2; image-rendering: pixelated; min-width: 0; object-fit: contain; }
</style>
