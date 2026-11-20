<script lang="ts">
  // Row of stat tiles (Stats, profile Overview): label bar + value bar in the real tile padding. Replaces the `–` placeholders.
  import Skeleton from './Skeleton.svelte';
  import SkeletonRegion from './SkeletonRegion.svelte';
  let { count = 5, big = 22 }: { count?: number; big?: number } = $props();
</script>

<SkeletonRegion class="sts">
  <div class="tiles">
    {#each Array(count) as _, i (i)}
      <div><Skeleton variant="bar" lh={17} h={10} w="58%" /><span class="v"><Skeleton variant="bar" lh={big === 22 ? 31 : 26} h={big - 6} w="46%" /></span></div>
    {/each}
  </div>
</SkeletonRegion>

<style>
  .tiles { display: grid; grid-template-columns: repeat(auto-fit, minmax(150px, 1fr)); margin: 0; border: 1px solid var(--line); overflow: hidden; }
  .tiles > div { padding: 12px 16px; background: var(--ink-1); border-right: 1px solid var(--line-soft); }
  .tiles > div:last-child { border-right: 0; }
  .v { display: block; margin-top: 4px; }
</style>
