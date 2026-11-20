<script lang="ts">
  // World cards (Worlds tab): 64 px icon, name, chips, meta, action row. Same grid and padding as Worlds.svelte.
  import Skeleton from './Skeleton.svelte';
  import SkeletonRegion from './SkeletonRegion.svelte';
  let { count = 2 }: { count?: number } = $props();
</script>

<SkeletonRegion class="wcs">
  <ul class="grid">
    {#each Array(count) as _, i (i)}
      <li class="card">
        <Skeleton variant="box" w={64} h={64} notch={10} />
        <div class="info">
          <Skeleton variant="bar" lh={26} h={15} w="46%" />
          <div class="chips"><Skeleton variant="box" w={78} h={22} radius={4} /><Skeleton variant="box" w={64} h={22} radius={4} /></div>
          <Skeleton variant="bar" lh={20} h={11} w="58%" />
        </div>
        <div class="acts"><Skeleton variant="box" w={92} h={28} radius={8} /><Skeleton variant="box" w={104} h={28} radius={8} /></div>
      </li>
    {/each}
  </ul>
</SkeletonRegion>

<style>
  .grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(min(100%, 420px), 1fr)); gap: 10px; list-style: none; margin: 0; padding: 0; }
  .card { display: grid; grid-template-columns: auto minmax(0, 1fr); gap: 4px 14px; padding: 12px; border: 1px solid var(--line); background: var(--ink-2); }
  .info { display: grid; gap: 6px; min-width: 0; align-content: start; }
  .chips { display: flex; gap: 6px; }
  .acts { grid-column: 1 / -1; display: flex; gap: 6px; padding-top: 8px; border-top: 1px solid var(--line-soft); margin-top: 4px; }
</style>
