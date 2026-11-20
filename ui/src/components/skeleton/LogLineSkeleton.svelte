<script lang="ts">
  // Lines of a log (monospace, 12.5 px / 1.55): bars of varied width, no characters.
  import Skeleton from './Skeleton.svelte';
  import SkeletonRegion from './SkeletonRegion.svelte';
  let { count = 14, lineHeight = 19, pad = '10px 14px', announce = true }: { count?: number; lineHeight?: number; pad?: string; announce?: boolean } = $props();
  const W = [78, 92, 64, 88, 46, 96, 71, 58, 84, 40, 90, 67, 80, 52];
</script>

<SkeletonRegion class="lls" {announce}>
  <div class="log" style:padding={pad}>
    {#each Array(count) as _, i (i)}<Skeleton variant="bar" lh={lineHeight} h={10} w="{W[i % W.length]}%" />{/each}
  </div>
</SkeletonRegion>

<style>
  .log { display: grid; }
</style>
