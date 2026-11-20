<script lang="ts">
  // Discover result rows (56 px icon, title, two description lines, tags, stats, install button). Same grid and padding as Discover.svelte.
  import Skeleton from './Skeleton.svelte';
  import SkeletonRegion from './SkeletonRegion.svelte';
  let { count = 6, announce = true, top = true, countLine = top }: { count?: number; announce?: boolean; top?: boolean; countLine?: boolean } = $props();
</script>

<SkeletonRegion class="srs" {announce}>
  {#if countLine}<Skeleton variant="bar" lh={19} h={11} w={96} />{/if}
  <ul class:top>
    {#each Array(count) as _, i (i)}
      <li class="row">
        <Skeleton variant="box" w={56} h={56} notch={9} />
        <div class="main">
          <Skeleton variant="bar" lh={22} h={15} w="34%" />
          <Skeleton variant="bar" lh={19} h={11} w="82%" />
          <Skeleton variant="bar" lh={20} h={16} w="22%" />
        </div>
        <div class="stats"><Skeleton variant="bar" lh={20} h={11} w={54} /><Skeleton variant="bar" lh={20} h={11} w={42} /></div>
        <Skeleton variant="box" w={132} h={36} radius={8} />
      </li>
    {/each}
  </ul>
</SkeletonRegion>

<style>
  ul { list-style: none; margin: 0; padding: 0; display: grid; }
  ul.top { border-top: 1px solid var(--line); }
  .row { display: grid; grid-template-columns: 56px minmax(0, 1fr) auto 132px; gap: 16px; align-items: center; padding: 12px 8px; border-bottom: 1px solid var(--line-soft); }
  .main { min-width: 0; display: grid; gap: 3px; }
  .stats { display: grid; gap: 4px; justify-items: end; }
  @media (max-width: 1100px) { .stats { display: none; } .row { grid-template-columns: 56px minmax(0, 1fr) 132px; } }
</style>
