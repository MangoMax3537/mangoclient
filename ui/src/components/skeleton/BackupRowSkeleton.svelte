<script lang="ts">
  // Backup rows (44 px world icon, title + chips, date + size, actions) in the bordered list of Backups.svelte.
  import Skeleton from './Skeleton.svelte';
  import SkeletonRegion from './SkeletonRegion.svelte';
  let { count = 4 }: { count?: number } = $props();
</script>

<SkeletonRegion class="brs">
  <ul class="rows">
    {#each Array(count) as _, i (i)}
      <li class="row">
        <Skeleton variant="box" w={44} h={44} notch={8} />
        <div class="t"><Skeleton variant="bar" lh={22} h={14} w="44%" /><span class="chips"><Skeleton variant="box" w={64} h={20} radius={4} /><Skeleton variant="box" w={86} h={20} radius={4} /></span></div>
        <div class="when"><Skeleton variant="bar" lh={19} h={11} w={116} /><Skeleton variant="bar" lh={19} h={11} w={52} /></div>
        <div class="acts"><Skeleton variant="box" w={96} h={28} radius={8} /><Skeleton variant="box" w={34} h={28} radius={8} /></div>
      </li>
    {/each}
  </ul>
</SkeletonRegion>

<style>
  .rows { list-style: none; margin: 0; padding: 0; border: 1px solid var(--line); border-radius: var(--r-m); background: var(--ink-2); }
  .row { display: grid; grid-template-columns: auto minmax(0, 1fr) auto auto; gap: 14px; align-items: center; padding: 10px 12px; border-bottom: 1px solid var(--line-soft); }
  .row:last-child { border-bottom: 0; }
  .t { display: grid; gap: 5px; min-width: 0; }
  .chips { display: flex; gap: 6px; }
  .when { display: grid; gap: 2px; justify-items: end; }
  .acts { display: flex; gap: 6px; }
  @media (max-width: 1040px) {
    .row { grid-template-columns: auto minmax(0, 1fr) auto; }
    .when { grid-column: 2; grid-row: 2; display: flex; gap: 12px; justify-items: start; }
    .acts { grid-column: 3; grid-row: 1 / span 2; flex-direction: column; }
  }
</style>
