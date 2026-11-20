<script lang="ts">
  // Generic list rows: an image box + two bars (+ optional trailing button box). Pass the real row's height and padding.
  import Skeleton from './Skeleton.svelte';
  import SkeletonRegion from './SkeletonRegion.svelte';
  interface Props {
    count?: number;
    /** Row height in px (fixed rows such as the virtual list's 62). Omit for padding-defined rows. */
    rowHeight?: number;
    img?: number;
    notch?: number;
    padding?: string;
    gap?: number;
    bars?: { w: string; h: number }[];
    /** Trailing box (a button) width in px. */
    tail?: number;
    announce?: boolean;
    /** Border colour token between rows. */
    border?: boolean;
    /** Draw the divider under the last row too (false inside a framed list). */
    lastBorder?: boolean;
  }
  let { count = 6, rowHeight, img = 40, notch = 0, padding = '0 10px 0 12px', gap = 12, bars = [{ w: '38%', h: 13 }, { w: '24%', h: 11 }], tail = 0, announce = true, border = true, lastBorder = true }: Props = $props();
</script>

<SkeletonRegion class="lrs" {announce}>
  <ul>
    {#each Array(count) as _, i (i)}
      <li class:bd={border} class:nolast={!lastBorder} style:height={rowHeight ? `${rowHeight}px` : undefined} style:padding style:gap="{gap}px">
        {#if img}<Skeleton variant="box" w={img} h={img} {notch} radius={6} />{/if}
        <span class="tx">{#each bars as b, k (k)}<Skeleton variant="bar" w={b.w} h={b.h} />{/each}</span>
        {#if tail}<Skeleton variant="box" w={tail} h={28} radius={8} />{/if}
      </li>
    {/each}
  </ul>
</SkeletonRegion>

<style>
  ul { list-style: none; margin: 0; padding: 0; }
  li { display: flex; align-items: center; }
  li.bd { border-bottom: 1px solid var(--line-soft); }
  li.nolast:last-child { border-bottom: 0; }
  .tx { flex: 1; min-width: 0; display: grid; gap: 8px; }
</style>
