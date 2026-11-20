<script lang="ts">
  // Screenshot grid (Media tab) and strip (Overview): 16:9 grey boxes in the real grid.
  import SkeletonRegion from './SkeletonRegion.svelte';
  let { count = 8, announce = true, min = 210, gap = 10 }: { count?: number; announce?: boolean; min?: number; gap?: number } = $props();
</script>

<SkeletonRegion class="sgs" {announce}>
  <ul style:grid-template-columns="repeat(auto-fill, minmax({min}px, 1fr))" style:gap="{gap}px">
    {#each Array(count) as _, i (i)}<li class="skel"></li>{/each}
  </ul>
</SkeletonRegion>

<style>
  ul { display: grid; list-style: none; margin: 0; padding: 0; }
  li { aspect-ratio: 16 / 9; border-radius: var(--r-m); }
</style>
