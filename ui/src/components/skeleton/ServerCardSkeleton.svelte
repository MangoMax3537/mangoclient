<script lang="ts">
  // Servers: a whole card (list still loading) or, with `status`, just the status area inside a real card (name, address and icon are known).
  import Skeleton from './Skeleton.svelte';
  import SkeletonRegion from './SkeletonRegion.svelte';
  let { count = 6, status = false }: { count?: number; status?: boolean } = $props();
</script>

{#if status}
  <!-- inside the real card: the MOTD line and the ping / players line -->
  <span class="st" aria-hidden="true">
    <Skeleton variant="bar" lh={20} h={12} w="62%" />
    <span class="ln"><Skeleton variant="circle" size={8} /><Skeleton variant="bar" lh={18} h={11} w={48} /><Skeleton variant="bar" lh={18} h={11} w={64} /></span>
    <Skeleton variant="bar" lh={18} h={11} w={44} />
  </span>
{:else}
  <SkeletonRegion class="scs">
    <ul class="grid">
      {#each Array(count) as _, i (i)}
        <li class="card">
          <Skeleton variant="box" w={64} h={64} radius={8} />
          <div class="info">
            <Skeleton variant="bar" lh={24} h={15} w="42%" />
            <Skeleton variant="bar" lh={19} h={11} w="34%" />
            <Skeleton variant="bar" lh={20} h={12} w="62%" />
            <span class="ln"><Skeleton variant="circle" size={8} /><Skeleton variant="bar" lh={18} h={11} w={48} /><Skeleton variant="bar" lh={18} h={11} w={64} /></span>
            <Skeleton variant="bar" lh={18} h={11} w={44} />
          </div>
          <Skeleton variant="box" w={84} h={36} notch={6} />
        </li>
      {/each}
    </ul>
  </SkeletonRegion>
{/if}

<style>
  .st { display: grid; gap: 2px; }
  .ln { display: flex; align-items: center; gap: 12px; }
  .grid { list-style: none; margin: 0; padding: 0; display: grid; grid-template-columns: repeat(auto-fill, minmax(430px, 1fr)); gap: 12px; }
  .card { display: grid; grid-template-columns: 64px minmax(0, 1fr) auto; gap: 14px; align-items: center; padding: 12px; background: var(--ink-2); border: 1px solid var(--line); border-radius: var(--r-m); }
  .info { min-width: 0; display: grid; gap: 2px; }
</style>
