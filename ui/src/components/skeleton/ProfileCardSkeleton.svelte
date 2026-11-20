<script lang="ts">
  // Library grid card: cover (16:10) + name and two facts lines. Same grid, notch and padding as Library.svelte.
  import Skeleton from './Skeleton.svelte';
  import SkeletonRegion from './SkeletonRegion.svelte';
  import { readLibraryLayout } from '../../lib/layoutHint';
  // `tags` / `grouped` default to what the Library showed last time
  const hint = readLibraryLayout();
  let { count = 6, tags = hint.tags, grouped = hint.grouped }: { count?: number; tags?: boolean; grouped?: boolean } = $props();
</script>

<SkeletonRegion class="pcs">
  {#if grouped}<div class="gh"><Skeleton variant="bar" lh={34} h={16} w={140} /></div>{/if}
  <ul class="grid">
    {#each Array(count) as _, i (i)}
      <li class="cell">
        <div class="card notch" style:--n="16px">
          <span class="cover skel"></span>
          <span class="info">
            <Skeleton variant="bar" lh={23} h={16} w="58%" />
            <Skeleton variant="bar" lh={19} h={11} w="44%" />
            <Skeleton variant="bar" lh={19} h={11} w="52%" />
            {#if tags}<span class="tg"><Skeleton variant="box" w={44} h={18} radius={4} /><Skeleton variant="box" w={52} h={18} radius={4} /></span>{/if}
          </span>
        </div>
      </li>
    {/each}
  </ul>
</SkeletonRegion>

<style>
  .grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(232px, 1fr)); gap: var(--s-4); list-style: none; margin: 0; padding: 0; }
  .card { display: grid; grid-template-rows: auto 1fr; width: 100%; background: var(--ink-2); }
  .gh { border-bottom: 1px solid var(--line); margin-bottom: var(--s-3); }
  .tg { display: flex; gap: 4px; margin-top: 4px; }
  .cover { display: block; aspect-ratio: 16 / 10; }
  .info { display: grid; align-content: start; gap: 2px; padding: 12px 14px 14px; }
</style>
