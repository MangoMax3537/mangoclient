<script lang="ts">
  // One drawn background: a wallpaper / custom image (ken-burns), the Minecraft panorama as a slowly rotating CSS 3D cube, or the surface
  // with the MangoClient icon. It stays a plain grey (pulsing) box until its image(s) have loaded AND decoded, then fades in (opacity only);
  // `onready` fires at that moment (Home waits for it before swapping layers). Resolution lives in PanoramaBackground / HomeBackground.
  import { decodeSoon, imgWatch } from '../lib/imgLoad';
  import { floorWallpaper, readLastWallpaper } from '../lib/bgFloor';
  import { wallpapers } from '../lib/wallpapers';
  import { PANORAMA_SECONDS, panoramaDelay, type ResolvedBackground } from '../lib/instanceIcon';

  interface Props {
    res: ResolvedBackground;
    look: { dim: number; blur: number };
    tint?: string | null;
    /** Start angle of the panorama (degrees). */
    angle: number;
    paused?: boolean;
    /** Full-window mode (behind the other views): a flat uniform scrim in the page colour, no hero gradient, no watermark. */
    full?: boolean;
    onready?: () => void;
    /** A layer inside the engine: transparent (the floor shows through while it loads), no skeleton, and a failure is reported instead of replaced. */
    bare?: boolean;
    /** Shown at once without a fade (the engine's floor). */
    instant?: boolean;
    onfail?: () => void;
  }
  let { res: want, look, tint = null, angle, paused = false, full = false, onready, bare = false, instant = false, onfail }: Props = $props();

  // the bundled wallpaper: what stands in for anything that is missing, broken or a flat surface (never a plain colour)
  const fb = floorWallpaper(wallpapers, readLastWallpaper());
  const fbRes = fb ? ({ kind: 'image', url: fb.url, scrim: fb.scrim, source: 'wallpaper' } as const) : null;
  let imgFailed = $state(false);
  let faceFailed = $state(false);
  $effect(() => { void want; imgFailed = false; faceFailed = false; });
  const broken = $derived((want.kind === 'image' && imgFailed) || (want.kind === 'panorama' && faceFailed));
  const flat = $derived(want.kind === 'plain' || want.kind === 'watermark');
  const res = $derived(!bare && fbRes && (broken || flat) ? fbRes : want);
  $effect(() => { if (broken && bare) onfail?.(); });
  const showUnder = $derived(!bare && !!fbRes && !(res.kind === 'image' && res.url === fbRes.url));

  const artKey = $derived(res.kind === 'image' ? res.url : res.kind === 'panorama' ? res.set.faces.join('|') : '');
  let artLoaded = $state(false);
  let faces = $state(0);
  let facesReady = $state(false);
  let seenKey = '';
  $effect.pre(() => { if (artKey !== seenKey) { seenKey = artKey; artLoaded = false; faces = 0; facesReady = false; } });
  const waiting = $derived((res.kind === 'image' && !artLoaded) || (res.kind === 'panorama' && !facesReady));

  let artEl: HTMLImageElement | undefined = $state();
  let faceEls: (HTMLImageElement | undefined)[] = $state([]);
  const decode = (el: HTMLImageElement | undefined) => decodeSoon(el);
  async function onArt() { const k = artKey; await decode(artEl); if (k === artKey) artLoaded = true; }
  async function onFace() {
    faces += 1;
    if (faces < 6) return;
    const k = artKey;
    await Promise.all(faceEls.map(decode));
    if (k === artKey) facesReady = true;
  }
  const seen = { onload: () => { void onFace(); }, onerror: () => { faceFailed = true; void onFace(); } };
  let fired = '';
  $effect(() => { if (!waiting && fired !== artKey + res.kind) { fired = artKey + res.kind; onready?.(); } });
  // face order = panorama_0..5: front, right, back, left, up, down
  const FACE_CLASS = ['front', 'right', 'back', 'left', 'up', 'down'] as const;
</script>

<div class="stage" class:full class:bare style:--sc={look.dim} style:--blur="{look.blur}px" style:--tint={tint ?? 'var(--accent-fill)'} aria-hidden="true">
  {#if showUnder && fbRes}<img class="under" src={fbRes.url} alt="" draggable="false" />{/if}
  {#if broken && bare}
    <!-- the engine's floor shows through -->
  {:else if res.kind === 'panorama'}
    <div class="scene" class:ready={!waiting}>
      <div class="cube" class:paused={paused} style:--a0="{angle}deg" style:--d="{panoramaDelay(angle)}" style:--dur="{PANORAMA_SECONDS}s">
        {#each res.set.faces as src, i (i)}
          <img bind:this={faceEls[i]} class="face {FACE_CLASS[i]}" {src} alt="" draggable="false" decoding="async" use:imgWatch={{ ...seen, src }} />
        {/each}
      </div>
    </div>
  {:else if res.kind === 'image'}
    <img bind:this={artEl} class="art" class:ready={artLoaded} class:instant src={res.url} alt="" draggable="false" decoding="async" use:imgWatch={{ src: res.url, onload: () => { void onArt(); }, onerror: () => { imgFailed = true; } }} />
  {/if}
  {#if !(bare && (waiting || broken))}<div class="scrim" class:plain={res.kind === 'plain' || res.kind === 'watermark'}></div>{/if}
</div>

<style>
  .stage { position: absolute; inset: 0; overflow: hidden; background: var(--hero-bg); container-type: size; }
  .stage.bare { background: transparent; }
  .under { position: absolute; inset: calc(var(--blur) * -2); width: calc(100% + var(--blur) * 4); height: calc(100% + var(--blur) * 4); object-fit: cover; filter: blur(var(--blur)); }
  .art.instant { opacity: 1; transition: none; }
  /* full-window mode: the page colour under a uniform scrim, so text on top stays readable in both themes */
  .stage.full:not(.bare) { background: var(--ink-0); }
  .stage.full .scrim, .stage.full .scrim.plain { background: color-mix(in srgb, var(--ink-0) calc(var(--sc) * 100%), transparent); }
  .art, .scene { opacity: 0; transition: opacity var(--dur-slow) var(--ease); }
  .art.ready, .scene.ready { opacity: 1; }
  .art { position: absolute; inset: calc(var(--blur) * -2); width: calc(100% + var(--blur) * 4); height: calc(100% + var(--blur) * 4); object-fit: cover; filter: blur(var(--blur)); transform-origin: 70% 40%; animation: kb 36s ease-out infinite alternate; }

  /* The camera sits in the middle of the cube: perspective p = half the stage width, cube edge = 2p, so the view is ~90 degrees wide. */
  .scene { --p: 50cqw; position: absolute; inset: calc(var(--blur) * -2 - 4px); perspective: var(--p); filter: blur(var(--blur)); }
  .cube {
    position: absolute; left: 50%; top: 50%; width: calc(var(--p) * 2); height: calc(var(--p) * 2); margin: calc(var(--p) * -1) 0 0 calc(var(--p) * -1);
    transform-style: preserve-3d; transform: translateZ(var(--p)) rotateX(-5deg) rotateY(var(--a0));
    animation: spin var(--dur) linear infinite; animation-delay: calc(var(--d) * 1s); will-change: transform;
 }
  .cube.paused { animation-play-state: paused; }
  .face { position: absolute; inset: 0; width: 100%; height: 100%; object-fit: fill; backface-visibility: hidden; }
  .front { transform: translateZ(calc(var(--p) * -1)) scale(1.003); }
  .right { transform: rotateY(-90deg) translateZ(calc(var(--p) * -1)) scale(1.003); }
  .back { transform: rotateY(-180deg) translateZ(calc(var(--p) * -1)) scale(1.003); }
  .left { transform: rotateY(-270deg) translateZ(calc(var(--p) * -1)) scale(1.003); }
  .up { transform: rotateX(-90deg) translateZ(calc(var(--p) * -1)) scale(1.003); }
  .down { transform: rotateX(90deg) translateZ(calc(var(--p) * -1)) scale(1.003); }

  /* --sc = overlay strength (0.3-0.9, default 0.6): left text side gets sc*1.5, the middle sc*1.2, the right sc*0.3 */
  .scrim { position: absolute; inset: 0; background: linear-gradient(90deg, rgba(var(--scrim), calc(var(--sc) * 1.5)) 0%, rgba(var(--scrim), calc(var(--sc) * 1.2)) 42%, rgba(var(--scrim), calc(var(--sc) * 0.3)) 100%), linear-gradient(0deg, rgba(var(--scrim), calc(var(--sc) * 1.08)), transparent 55%); }
  .scrim.plain { background: linear-gradient(90deg, rgba(var(--scrim), 0.35), transparent 70%); }
  @keyframes spin { from { transform: translateZ(var(--p)) rotateX(-5deg) rotateY(0deg); } to { transform: translateZ(var(--p)) rotateX(-5deg) rotateY(360deg); } }
  @keyframes kb { to { transform: scale(1.08) translate3d(-1%, -1%, 0); } }
  @media (prefers-reduced-motion: reduce) {
    .cube { animation: none; }
    .art { animation: none; }
 }
</style>
