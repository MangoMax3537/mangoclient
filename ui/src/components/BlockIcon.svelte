<script lang="ts">
  import type { BlockFace } from '../lib/types';

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
</script>

<span
  class="bt" class:notch={notch > 0} style:--n="{notch}px" style:--tint={tint}
  style:width={fill ? '100%' : `${size}px`} style:height={fill ? '100%' : `${size}px`}
  style:border-radius={notch ? undefined : `${radius}px`}
  role={label ? 'img' : undefined} aria-label={label || undefined} aria-hidden={label ? undefined : 'true'}
>
  <span class="scene">
    <span class="shadow"></span>
    <span class="cube">
      <span class="f top"><img class="px" src={faces.top} alt="" draggable="false" onerror={() => onfail?.()} /></span>
      <span class="f left"><img class="px" src={faces.left} alt="" draggable="false" onerror={() => onfail?.()} /></span>
      <span class="f right"><img class="px" src={faces.right} alt="" draggable="false" onerror={() => onfail?.()} /></span>
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
  .scene { --w: 62cqmin; --a: calc(var(--w) * 0.57735); position: absolute; left: 50%; top: 50%; width: var(--w); height: calc(var(--a) * 2); transform: translate(-50%, -52%); }
  .cube { position: absolute; inset: 0; transition: transform var(--dur) var(--ease); }
  /* hexagon underlay: closes the hairline seams between faces and gives a dark contour */
  .cube::before { content: ''; position: absolute; inset: 0; background: rgba(0, 0, 0, 0.55); clip-path: polygon(50% 0, 100% 25%, 100% 75%, 50% 100%, 0 75%, 0 25%); }
  .f { position: absolute; width: var(--a); height: var(--a); transform-origin: 0 0; overflow: hidden; }
  .f img { display: block; width: 100%; height: 100%; object-fit: fill; }
  .f::after { content: ''; position: absolute; inset: 0; }
  .top { left: 0; top: calc(var(--a) * 0.5); transform: matrix(0.866025, -0.5, 0.866025, 0.5, 0, 0); }
  .left { left: 0; top: calc(var(--a) * 0.5); transform: matrix(0.866025, 0.5, 0, 1, 0, 0); }
  .right { left: 50%; top: var(--a); transform: matrix(0.866025, -0.5, 0, 1, 0, 0); }
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
