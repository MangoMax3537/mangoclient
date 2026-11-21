<script lang="ts">
  // Servers view: the rows while the list still loads (favicon box, name, address, status, star, Join).
  import Skeleton from './Skeleton.svelte';
  import SkeletonRegion from './SkeletonRegion.svelte';
  let { count = 6 }: { count?: number } = $props();
</script>

<SkeletonRegion class="scs">
  <ul class="list">
    {#each Array(count) as _, i (i)}
      <li class="row">
        <Skeleton variant="box" w={56} h={56} radius={6} />
        <div class="info">
          <Skeleton variant="bar" lh={25} h={15} w="38%" />
          <Skeleton variant="bar" lh={19} h={11} w="30%" />
        </div>
        <Skeleton variant="bar" lh={20} h={12} w={150} />
        <Skeleton variant="box" w={32} h={32} />
        <Skeleton variant="box" w={84} h={36} notch={6} />
      </li>
    {/each}
  </ul>
</SkeletonRegion>

<style>
  .list { list-style: none; margin: 0; padding: 0; display: grid; gap: 10px; }
  .row { display: grid; grid-template-columns: 56px minmax(0, 1fr) 150px 32px auto; gap: 16px; align-items: center; padding: 12px 14px; background: var(--ink-2); }
  .info { min-width: 0; display: grid; gap: 2px; }
</style>
