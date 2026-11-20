<script lang="ts">
  // First-screen skeleton of a view (boot, or while its chunk loads): the same page padding and block sizes as the real screen, so the
  // content replaces it without shifting. Shapes only, no text or icons.
  import Skeleton from './skeleton/Skeleton.svelte';
  import SkeletonRegion from './skeleton/SkeletonRegion.svelte';
  import HeroSkeleton from './skeleton/HeroSkeleton.svelte';
  import JumpBackSkeleton from './skeleton/JumpBackSkeleton.svelte';
  import ProfileCardSkeleton from './skeleton/ProfileCardSkeleton.svelte';
  import SearchResultSkeleton from './skeleton/SearchResultSkeleton.svelte';
  import ServerCardSkeleton from './skeleton/ServerCardSkeleton.svelte';
  import StatTileSkeleton from './skeleton/StatTileSkeleton.svelte';
  import ListRowSkeleton from './skeleton/ListRowSkeleton.svelte';
  import type { View } from '../lib/stores.svelte';

  let { view = 'home' }: { view?: View } = $props();
</script>

{#snippet head(hint = true)}
  <div class="head"><Skeleton variant="bar" lh={38} h={30} w={220} radius={6} />{#if hint}<Skeleton variant="bar" lh={20} h={11} w={150} />{/if}</div>
{/snippet}

{#if view === 'home'}
  <div class="home">
    <HeroSkeleton />
    <div class="body">
      <div class="sec"><div class="sh strip-h"><Skeleton variant="bar" lh={30} h={18} w={160} /></div><JumpBackSkeleton /></div>
      <div class="split">
        <div class="sec"><div class="sh"><Skeleton variant="bar" lh={30} h={18} w={140} /></div><ListRowSkeleton count={4} img={32} rowHeight={50} padding="0" bars={[{ w: '34%', h: 13 }, { w: '22%', h: 10 }]} tail={32} /></div>
        <div class="sec pt"><div class="sh"><Skeleton variant="bar" lh={30} h={18} w={140} /></div><span class="bigsk"><Skeleton variant="bar" h={34} w={130} radius={6} /></span><Skeleton variant="box" w="100%" h={64} radius={4} /><div class="axissk"><Skeleton variant="bar" lh={17} h={9} w={16} /><Skeleton variant="bar" lh={17} h={9} w={10} /></div></div>
      </div>
    </div>
  </div>
{:else if view === 'library'}
  <div class="pg">
    {@render head()}
    <div class="bar"><Skeleton variant="box" h={36} w="100%" radius={8} style="flex:1" /><Skeleton variant="box" w={150} h={36} radius={8} /><Skeleton variant="box" w={78} h={34} radius={8} /></div>
    <ProfileCardSkeleton count={6} />
  </div>
{:else if view === 'discover'}
  <div class="pg">
    {@render head()}
    <Skeleton variant="box" w={380} h={36} radius={8} />
    <div class="bar"><Skeleton variant="box" h={36} w="100%" radius={8} style="flex:1" /><Skeleton variant="box" w={140} h={36} radius={8} /><Skeleton variant="box" w={140} h={36} radius={8} /></div>
    <SearchResultSkeleton count={6} />
  </div>
{:else if view === 'servers'}
  <div class="pg">{@render head()}<ServerCardSkeleton count={6} /></div>
{:else if view === 'stats'}
  <div class="pg">{@render head(false)}<StatTileSkeleton /><Skeleton variant="box" w="100%" h={240} radius={8} /></div>
{:else if view === 'settings'}
  <div class="pg narrow">
    {@render head(false)}
    {#each [1, 3, 2, 3] as rows, i (i)}
      <div class="ss"><Skeleton variant="bar" lh={37} h={11} w={120} /><ListRowSkeleton count={rows} img={0} rowHeight={64} padding="0" gap={24} bars={[{ w: '36%', h: 14 }, { w: '56%', h: 11 }]} tail={40} announce={false} /></div>
    {/each}
  </div>
{:else}
  <div class="page">
    <HeroSkeleton variant="profile" />
    <Skeleton variant="box" w="100%" h={42} radius={8} />
    <SkeletonRegion class="pbody"><div class="ov"><Skeleton variant="box" w="100%" h={64} radius={8} /><Skeleton variant="box" w="100%" h={120} radius={8} /></div></SkeletonRegion>
  </div>
{/if}

<style>
  .home { height: 100%; overflow: hidden; display: flex; flex-direction: column; }
  .body { display: grid; gap: var(--s-6); padding: var(--s-5) 32px 32px 44px; }
  .sec { display: block; }
  .sec.pt { display: grid; gap: var(--s-2); align-content: start; }
  .sh { height: 30px; margin-bottom: var(--s-2); display: flex; align-items: center; }
  .sec.pt .sh { margin-bottom: 0; }
  .sh.strip-h { margin-bottom: var(--s-3); }
  .bigsk { display: flex; align-items: center; height: 40px; margin: 4px 0 var(--s-2); }
  .axissk { display: flex; justify-content: space-between; }
  .split { display: grid; grid-template-columns: minmax(0, 1fr) minmax(0, 1fr); gap: var(--s-7); }
  .pg { height: 100%; overflow: hidden; padding: var(--s-5) 32px 32px; display: flex; flex-direction: column; gap: var(--s-4); }
  .pg.narrow { max-width: 800px; display: grid; align-content: start; gap: 24px; }
  .head { display: grid; gap: 4px; }
  .bar { display: flex; gap: var(--s-3); align-items: center; }
  .page { display: flex; flex-direction: column; height: 100%; padding: 16px 20px 0; gap: 12px; }
  .ov { display: grid; gap: 22px; max-width: 980px; }
  .ss { display: grid; border-top: 1px solid var(--line); }
</style>
