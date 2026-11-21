<script lang="ts">
  // The "jump back in" strip of Home: 212 px notched cards (16:10 art, name, two meta lines).
  import Skeleton from './Skeleton.svelte';
  import SkeletonRegion from './SkeletonRegion.svelte';
  let { count = 5 }: { count?: number } = $props();
</script>

<SkeletonRegion class="jbs">
  <ul class="strip">
    {#each Array(count) as _, i (i)}
      <li class="card notch" style:--n="14px">
        <span class="art skel"></span>
        <span class="t"><Skeleton variant="bar" lh={19} h={14} w="62%" /><Skeleton variant="bar" lh={19} h={11} w="48%" /><Skeleton variant="bar" lh={19} h={11} w="56%" /></span>
      </li>
    {/each}
  </ul>
</SkeletonRegion>

<style>
  .strip { list-style: none; margin: 0 -32px 0 -4px; padding: 4px 32px 18px 4px; display: flex; gap: var(--s-3); overflow: hidden; }
  .card { flex: none; width: 212px; background: var(--ink-2); display: grid; gap: 2px; padding-bottom: 18px; }
  .art { display: block; aspect-ratio: 16 / 10; margin-bottom: 10px; }
  .t { display: grid; gap: 2px; padding: 0 16px 0 14px; }
</style>
