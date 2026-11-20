<script lang="ts">
  import type { BlockFace } from '../lib/types';
  import { imgWatch } from '../lib/imgLoad';

  interface Props {
    /** Face image URLs (mangoimg://block/<id>/<face>). */
    faces: Record<BlockFace, string>;
    size?: number;
    fill?: boolean;
    radius?: number;
    /** Notched corner size in px (signature shape). Overrides radius. */
    notch?: number;
    /** CSS colour (a token like `var(--sky)` or the instance colour) the tile surface is derived from. */
    tint?: string | null;
    label?: string;
    /** Called when a face image cannot be loaded. */
    onfail?: () => void;
  }
  let { faces, size = 40, fill = false, radius = 6, notch = 0, tint = null, label = '', onfail }: Props = $props();

  // The tile is a plain grey box until all three faces have loaded, then the cube fades in (opacity only). A failed face reports onfail.
  let done = $state<Record<BlockFace, boolean>>({ top: false, left: false, right: false });
  let instant = $state(false);
  let seen = '';
  $effect.pre(() => { const k = `${faces.top}|${faces.left}|${faces.right}`; if (k !== seen) { seen = k; done = { top: false, left: false, right: false }; instant = false; } });
  const ready = $derived(done.top && done.left && done.right);
  const watch = (face: BlockFace) => ({
    src: faces[face],
    onload: (i: boolean) => { if (i && !ready) instant = true; done = { ...done, [face]: true }; },
    onerror: () => onfail?.(),
  });
</script>

<span
  class="bt" class:notch={notch > 0} style:--n="{notch}px" style:--tint={tint}
  style:width={fill ? '100%' : `${size}px`} style:height={fill ? '100%' : `${size}px`}
  style:border-radius={notch ? undefined : `${radius}px`}
  role={label ? 'img' : undefined} aria-label={label || undefined} aria-hidden={label ? undefined : 'true'}
>
  {#if !ready}<span class="skel fill" aria-hidden="true"></span>{/if}
  <span class="scene" class:ready class:instant>
    <span class="shadow"></span>
    <span class="cube">
      <span class="f top"><img class="px" src={faces.top} alt="" decoding="async" draggable="false" use:imgWatch={watch('top')} /></span>
      <span class="f left"><img class="px" src={faces.left} alt="" decoding="async" draggable="false" use:imgWatch={watch('left')} /></span>
      <span class="f right"><img class="px" src={faces.right} alt="" decoding="async" draggable="false" use:imgWatch={watch('right')} /></span>
    </span>
  </span>
</span>

<style>
  /* Isometric cube from three flat faces. Hexagon: width W, edge a = W / sqrt(3), height 2a. Each face is an a x a image sheared by
     an affine matrix (columns = where the image x and y axes go), so edges are exact and the pixel art stays crisp. */
  .bt {
    display: block; flex: none; position: relative; overflow: hidden; container-type: size;
    background:
      radial-gradient(72% 62% at 50% 44%, color-mix(in srgb, var(--tint, var(--sky)) 26%, transparent), transparent 72%),
      color-mix(in srgb, var(--ink-3) 80%, var(--tint, var(--sky)));
  }
  .fill { position: absolute; inset: 0; display: block; }
  .scene { opacity: 0; transition: opacity var(--dur) var(--ease); }
  .scene.ready { opacity: 1; }
  .scene.instant { transition: none; }
  /* Geometry in whole device-independent pixels: the edge length `a` is rounded to 1px where CSS round() exists, and the scene is placed with
     rounded offsets instead of a fractional translate(-50%), so face edges land on the same sub-pixel phase at every size and zoom. */
  .scene { --a: calc(62cqmin * 0.57735); position: absolute; width: calc(var(--a) * 1.732051); height: calc(var(--a) * 2); left: 50%; top: 50%; transform: translate(-50%, -52%); }
  @supports (width: round(down, 1px, 1px)) {
    .scene { --a: round(nearest, calc(62cqmin * 0.57735), 1px); transform: none;
      left: round(down, calc(50cqw - var(--a) * 0.866025), 1px); top: round(down, calc(50cqh - var(--a) * 1.04), 1px); }
  }
  .cube { position: absolute; inset: 0; transition: transform var(--dur) var(--ease); }
  /* Backing silhouette under the three faces, in a neutral that belongs to the surface (not black): whatever anti-aliasing leaves between
     two faces shows this instead of a dark hairline. */
  .cube::before { content: ''; position: absolute; inset: 0; background: color-mix(in srgb, var(--tint, var(--sky)) 28%, #34312c); }
  /* Everything is clipped to the exact hexagon, so each face can bleed 1px over every edge (its own outermost texel row, stretched) without
     changing the silhouette: shared edges overlap instead of meeting, so no anti-aliased gap can appear between two faces. */
  .cube { clip-path: polygon(50% 0, 100% 25%, 100% 75%, 50% 100%, 0 75%, 0 25%); }
  .f { position: absolute; width: calc(var(--a) + 2px); height: calc(var(--a) + 2px); transform-origin: 0 0; overflow: hidden; backface-visibility: hidden; }
  .f img { display: block; width: 100%; height: 100%; object-fit: fill; }
  .f::after { content: ''; position: absolute; inset: 0; }
  /* matrix(a b c d tx ty): the shear of each face, plus the translation that moves its origin back by the 1px bleed along its own axes */
  .top { left: 0; top: calc(var(--a) * 0.5); transform: matrix(0.866025, -0.5, 0.866025, 0.5, -1.732, 0); }
  .left { left: 0; top: calc(var(--a) * 0.5); transform: matrix(0.866025, 0.5, 0, 1, -0.866, -1.5); }
  .right { left: 50%; top: var(--a); transform: matrix(0.866025, -0.5, 0, 1, -0.866, -0.5); }
  /* Minecraft face shading: top 100%, left 80%, right 60% */
  .top::after { background: linear-gradient(135deg, rgba(255, 255, 255, 0.2), rgba(255, 255, 255, 0) 55%); }
  .left::after { background: rgba(0, 0, 0, 0.2); }
  .right::after { background: rgba(0, 0, 0, 0.4); }
  .shadow {
    position: absolute; left: -6%; width: 112%; top: calc(var(--a) * 1.5); height: calc(var(--a) * 0.7);
    background: radial-gradient(closest-side, rgba(0, 0, 0, 0.5), rgba(0, 0, 0, 0.18) 62%, transparent);
    transition: transform var(--dur) var(--ease), opacity var(--dur) var(--ease);
  }
  .bt:hover .cube, :global(:is(button, .card, li):hover) .bt .cube { transform: translateY(-6cqmin); }
  .bt:hover .shadow, :global(:is(button, .card, li):hover) .bt .shadow { transform: scale(0.88); opacity: 0.7; }
  @media (prefers-reduced-motion: reduce) { .cube, .shadow { transition: none; } }
</style>
