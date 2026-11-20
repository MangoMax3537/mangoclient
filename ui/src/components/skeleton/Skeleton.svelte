<script lang="ts">
  // The one skeleton shape. Flat grey (token --skel), pulses softly, no icon / logo / text inside, always aria-hidden.
  //   line / bar : a text bar (`w` width, default 100% / 60%), 12 / 14 px high
  //   box        : a rectangle (`w`, `h`, optional notch)
  //   circle / avatar : round, `size` px (avatar defaults to 40)
  //   card       : a surface (ink-2) container for more shapes (children); the shapes inside pulse
  //   row        : a flex row for shapes (children)
  import type { Snippet } from 'svelte';

  interface Props {
    variant?: 'line' | 'bar' | 'box' | 'circle' | 'avatar' | 'card' | 'row';
    w?: string | number;
    h?: string | number;
    size?: number;
    /** Notched corner in px (the app's signature shape). */
    notch?: number;
    radius?: number | string;
    /** Line-box height for line / bar: the shape is centred in a box of this height, so it takes the place of a real text line. */
    lh?: number;
    class?: string;
    style?: string;
    children?: Snippet;
  }
  let { variant = 'box', w, h, size, notch = 0, radius, lh, class: cls = '', style = '', children }: Props = $props();
  const px = (v: string | number | undefined): string | undefined => (typeof v === 'number' ? `${v}px` : v);
  const dim = $derived.by(() => {
    if (variant === 'circle' || variant === 'avatar') { const s = size ?? (variant === 'avatar' ? 40 : 16); return { w: `${s}px`, h: `${s}px` }; }
    if (variant === 'line') return { w: px(w) ?? '100%', h: px(h) ?? '12px' };
    if (variant === 'bar') return { w: px(w) ?? '60%', h: px(h) ?? '14px' };
    return { w: px(w), h: px(h) };
  });
</script>

{#if variant === 'card' || variant === 'row'}
  <div class="sk-{variant} {cls}" class:notch={notch > 0} style:--n="{notch}px" style:width={dim.w} style:height={dim.h} {style} aria-hidden="true">{@render children?.()}</div>
{:else if lh && (variant === 'line' || variant === 'bar')}
  <span class="sk-lh" style:height="{lh}px" style:width={dim.w} aria-hidden="true"><span class="skel sk-{variant} {cls}" style:width="100%" style:height={dim.h} {style}></span></span>
{:else}
  <span
    class="skel sk-{variant} {cls}" class:notch={notch > 0} style:--n="{notch}px" style:width={dim.w} style:height={dim.h} {style} aria-hidden="true"
  ></span>
{/if}

<style>
  .sk-lh { display: flex; align-items: center; max-width: 100%; flex: none; }
  .sk-line, .sk-bar, .sk-box, .sk-circle, .sk-avatar { display: block; flex: none; max-width: 100%; }
  .sk-card { background: var(--ink-2); display: grid; gap: var(--s-3); }
  .sk-row { display: flex; align-items: center; gap: var(--s-3); }
</style>
