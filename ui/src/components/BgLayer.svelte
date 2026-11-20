<script lang="ts">
  // One drawn background: a wallpaper / custom image (ken-burns), the Minecraft panorama as a slowly rotating CSS 3D cube, or the surface
  // with the MangoClient icon. It stays a plain grey (pulsing) box until its image(s) have loaded AND decoded, then fades in (opacity only);
  // `onready` fires at that moment (Home waits for it before swapping layers). Resolution lives in PanoramaBackground / HomeBackground.
  import { decodeSoon, imgWatch } from '../lib/imgLoad';
  import { brandIcon } from '../lib/fallbackImage';
  import { PANORAMA_SECONDS, panoramaDelay, type ResolvedBackground } from '../lib/instanceIcon';

  interface Props {
    res: ResolvedBackground;
    look: { dim: number; blur: number };
    tint?: string | null;
    /** Start angle of the panorama (degrees). */
    angle: number;
    paused?: boolean;
    onready?: () => void;
  }
  let { res: want, look, tint = null, angle, paused = false, onready }: Props = $props();

  let imgFailed = $state(false);
  $effect(() => { void want; imgFailed = false; });
  const res = $derived(want.kind === 'image' && imgFailed ? ({ kind: 'watermark' } as const) : want);

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
  const seen = { onload: () => { void onFace(); }, onerror: () => { void onFace(); } };
  let fired = '';
  $effect(() => { if (!waiting && fired !== artKey + res.kind) { fired = artKey + res.kind; onready?.(); } });
  // face order = panorama_0..5: front, right, back, left, up, down
  const FACE_CLASS = ['front', 'right', 'back', 'left', 'up', 'down'] as const;
</script>

<div class="stage" style:--sc={look.dim} style:--blur="{look.blur}px" style:--tint={tint ?? 'var(--accent-fill)'} aria-hidden="true">
  {#if res.kind === 'panorama'}
    <div class="scene" class:ready={!waiting}>
      <div class="cube" class:paused={paused} style:--a0="{angle}deg" style:--d="{panoramaDelay(angle)}" style:--dur="{PANORAMA_SECONDS}s">
        {#each res.set.faces as src, i (i)}
          <img bind:this={faceEls[i]} class="face {FACE_CLASS[i]}" {src} alt="" draggable="false" decoding="async" use:imgWatch={{ ...seen, src }} />
        {/each}
      </div>
    </div>
  {:else if res.kind === 'image'}
    <img bind:this={artEl} class="art" class:ready={artLoaded} src={res.url} alt="" draggable="false" decoding="async" use:imgWatch={{ src: res.url, onload: () => { void onArt(); }, onerror: () => { imgFailed = true; } }} />
  {:else}
    <div class="surface" class:glow={res.kind === 'watermark'}></div>
    {#if res.kind === 'watermark'}<img class="mark px" src={brandIcon(512)} alt="" draggable="false" />{/if}
  {/if}
  {#if waiting}<div class="wait skel"></div>{/if}
  <div class="scrim" class:plain={res.kind === 'plain' || res.kind === 'watermark'}></div>
</div>

<style>
  .stage { position: absolute; inset: 0; overflow: hidden; background: var(--hero-bg); container-type: size; }
  .wait { position: absolute; inset: 0; }
  .art, .scene { opacity: 0; transition: opacity var(--dur-slow) var(--ease); }
  .art.ready, .scene.ready { opacity: 1; }
  .art { position: absolute; inset: calc(var(--blur) * -2); width: calc(100% + var(--blur) * 4); height: calc(100% + var(--blur) * 4); object-fit: cover; filter: blur(var(--blur)); transform-origin: 70% 40%; animation: kb 36s ease-out infinite alternate; }
  .surface { position: absolute; inset: 0; background: var(--hero-bg); }
  .surface.glow { background: radial-gradient(90% 120% at 82% 38%, color-mix(in srgb, var(--tint) 20%, transparent), transparent 62%), var(--hero-bg); }
  .mark { position: absolute; right: 6%; top: 50%; height: 104%; width: auto; aspect-ratio: 1; opacity: 0.09; transform: translateY(-50%); animation: drift 40s ease-out infinite alternate; }

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
  @keyframes drift { to { transform: translateY(-50%) scale(1.04); } }
  @media (prefers-reduced-motion: reduce) {
    .cube { animation: none; }
    .art, .mark { animation: none; }
 }
</style>
