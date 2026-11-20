<script lang="ts">
  // First-screen skeleton of a profile tab while its chunk (or data) loads: same container geometry as the real tab.
  import Skeleton from './Skeleton.svelte';
  import SkeletonRegion from './SkeletonRegion.svelte';
  import ListRowSkeleton from './ListRowSkeleton.svelte';
  import WorldCardSkeleton from './WorldCardSkeleton.svelte';
  import BackupRowSkeleton from './BackupRowSkeleton.svelte';
  import LogLineSkeleton from './LogLineSkeleton.svelte';
  import ScreenshotGridSkeleton from './ScreenshotGridSkeleton.svelte';
  let { tab, bare = false }: { tab: 'overview' | 'content' | 'media' | 'backups' | 'logs' | 'settings'; bare?: boolean } = $props();
</script>

{#if tab === 'content'}
  <div class="ct">
    <SkeletonRegion class="bar"><div class="b"><Skeleton variant="box" w={300} h={36} radius={8} /><Skeleton variant="box" h={36} w="100%" radius={8} style="flex:1;min-width:160px" /><Skeleton variant="box" w={120} h={36} radius={8} /><Skeleton variant="box" w={110} h={36} notch={6} /></div></SkeletonRegion>
    <div class="list"><ListRowSkeleton count={8} rowHeight={62} img={40} announce={false} tail={34} /></div>
  </div>
{:else if tab === 'media'}
  <div class="md">
    <div class="hd"><Skeleton variant="bar" lh={22} h={11} w={90} /></div>
    <WorldCardSkeleton count={2} />
    <div class="hd g"><Skeleton variant="bar" lh={22} h={11} w={110} /></div>
    <ScreenshotGridSkeleton count={8} announce={false} />
  </div>
{:else if tab === 'backups'}
  <div class="bk">
    <div class="hd between"><Skeleton variant="bar" lh={36} h={11} w={110} /><Skeleton variant="box" w={150} h={36} notch={6} /></div>
    <BackupRowSkeleton count={4} />
  </div>
{:else if tab === 'logs'}
  {#snippet logsBody()}
    <ul class="files">{#each Array(6) as _, i (i)}<li><Skeleton variant="bar" lh={17} h={11} w="72%" /><Skeleton variant="bar" lh={16} h={9} w="54%" /></li>{/each}</ul>
    <div class="view"><div class="hd between"><Skeleton variant="bar" lh={36} h={12} w={180} /><Skeleton variant="box" w={220} h={36} radius={8} /></div><div class="out"><LogLineSkeleton count={16} lineHeight={20} pad="0 12px" announce={false} /></div></div>
  {/snippet}
  {#if bare}{@render logsBody()}{:else}<div class="lg">{@render logsBody()}</div>{/if}
{:else}
  <SkeletonRegion class="tsk"><div class="rows">{#each Array(5) as _, i (i)}<div class="r"><div class="tx"><Skeleton variant="bar" lh={22} h={14} w={180} /><Skeleton variant="bar" lh={20} h={11} w={300} /></div><Skeleton variant="box" w={120} h={36} radius={8} /></div>{/each}</div></SkeletonRegion>
{/if}

<style>
  .ct { height: 100%; display: flex; flex-direction: column; gap: 10px; min-height: 0; }
  .b { display: flex; flex-wrap: wrap; gap: 8px; align-items: center; }
  .list { flex: 1; border: 1px solid var(--line); border-radius: var(--r-m); overflow: hidden; min-height: 0; background: var(--ink-1); }
  .hd { display: flex; align-items: center; margin-bottom: 10px; min-height: 22px; }
  .hd.between { justify-content: space-between; }
  .hd.g { margin-top: var(--s-6); }
  .lg { height: 100%; display: grid; grid-template-columns: 260px minmax(0, 1fr); gap: 14px; min-height: 0; }
  .files { list-style: none; margin: 0; padding: 0; display: grid; gap: 4px; align-content: start; }
  .files li { display: grid; gap: 3px; padding: 8px 10px; }
  .view { display: flex; flex-direction: column; gap: 8px; min-height: 0; }
  .out { flex: 1; min-height: 0; background: var(--ink-1); border: 1px solid var(--line); border-radius: var(--r-m); overflow: hidden; }
  .rows { display: grid; max-width: 760px; }
  .r { display: flex; justify-content: space-between; align-items: center; gap: 24px; padding: 12px 0; border-bottom: 1px solid var(--line-soft); }
  .tx { display: grid; }
</style>
