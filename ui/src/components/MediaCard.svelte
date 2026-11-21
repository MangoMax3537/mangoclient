<script lang="ts">
  // One notched card of a MediaStrip: image area (16:10), name, then info lines. Used for instances and servers alike, so both look the same.
  // Nothing inside is clipped: the card keeps real padding at the bottom and every line ends in an ellipsis instead of being cut by the notch.
  import PlayIcon from 'phosphor-svelte/lib/PlayIcon';
  import type { Snippet } from 'svelte';
  
  interface Props {
    title: string;
    art: Snippet;
    /** Info lines under the name (InfoLine rows). */
    lines?: Snippet;
    selected?: boolean;
    live?: boolean;
    dimmed?: boolean;
    pressed?: boolean;
    /** Spread onto the main button (click / dblclick / keydown handlers). */
    handlers?: Record<string, unknown>;
    onclick?: () => void;
    /** A small Play badge in the top-right corner: a pure visual cue (the whole card is the button). Servers only; instance cards never show it. */
    playBadge?: boolean;
    n?: number;
    ariaLabel?: string;
  }
  let { title, art, lines, selected = false, live = false, dimmed = false, pressed, handlers = {}, onclick, playBadge = false, n = 14, ariaLabel }: Props = $props();
</script>

<div class="card notch frame" class:sel={selected} class:live class:dimmed style:--n="{n}px" data-media-card>
  <button class="open" type="button" aria-pressed={pressed} aria-label={ariaLabel} {...handlers} {onclick}>
    <span class="art">{@render art()}</span>
    <span class="nm">{title}</span>
    {#if lines}<span class="lines">{@render lines()}</span>{/if}
  </button>
  {#if playBadge}<span class="go notch" style:--n="8px" aria-hidden="true"><PlayIcon weight="bold" size={16} /></span>{/if}
</div>

<style>
  .card { position: relative; flex: none; width: 212px; background: var(--ink-2); transition: transform var(--dur) var(--ease); }
  .card:hover { transform: translateY(-2px); }
  .card.sel { --frame: var(--accent-fill); }
  .card.dimmed .art { opacity: 0.55; }
  .open { display: grid; width: 100%; text-align: left; gap: 2px; padding-bottom: 18px; }
  .open:focus-visible { outline: 2px solid var(--focus); outline-offset: -4px; }
  .art { display: block; position: relative; overflow: hidden; aspect-ratio: 16 / 10; margin-bottom: 6px; }
  /* line-height >= 1.3 with real padding above and below: `overflow: hidden` (needed for the ellipsis) must never touch the glyphs (the cap tops were cut) */
  .nm { position: relative; z-index: 1; font: 700 var(--fs-lg)/1.35 var(--font-display); padding: 4px 16px 3px 14px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .lines { display: grid; gap: 1px; padding: 0 16px 0 14px; min-width: 0; font: var(--fs-sm)/1.4 var(--font-mono); color: var(--paper-dim); }
  .card.live .nm::after { content: ''; display: inline-block; width: 7px; height: 7px; margin-left: 8px; background: var(--leaf); animation: pulse 1.6s ease-in-out infinite; }
  .go { position: absolute; right: 8px; top: 8px; width: 32px; height: 32px; display: grid; place-items: center; background: var(--accent-fill); color: var(--on-fill); pointer-events: none; z-index: 2; }
  @keyframes pulse { 50% { opacity: 0.4; } }
</style>
