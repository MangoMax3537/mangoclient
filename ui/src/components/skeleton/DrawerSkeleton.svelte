<script lang="ts">
  // Body of the project drawer below the header: gallery image (16:9), description paragraphs.
  import Skeleton from './Skeleton.svelte';
  import SkeletonRegion from './SkeletonRegion.svelte';
</script>

<SkeletonRegion class="ds">
  <div class="body">
    <span class="big skel"></span>
    <div class="thumbs">{#each Array(4) as _, i (i)}<Skeleton variant="box" w={84} h={47} radius={4} />{/each}</div>
    <div class="p"><Skeleton variant="bar" lh={22} h={14} w="38%" /><Skeleton variant="bar" lh={22} h={12} w="96%" /><Skeleton variant="bar" lh={22} h={12} w="90%" /><Skeleton variant="bar" lh={22} h={12} w="62%" /></div>
  </div>
</SkeletonRegion>

<style>
  .body { margin-top: 16px; }
  .big { display: block; width: 100%; aspect-ratio: 16 / 9; }
  .thumbs { display: flex; gap: 6px; margin-top: 8px; }
  .p { display: grid; margin-top: 14px; }
</style>
