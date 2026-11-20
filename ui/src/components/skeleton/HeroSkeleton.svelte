<script lang="ts">
  // Hero stage of Home (variant 'home') and of the profile page (variant 'profile'): same container, padding and block sizes as the real hero.
  import Skeleton from './Skeleton.svelte';
  import SkeletonRegion from './SkeletonRegion.svelte';
  let { variant = 'home' }: { variant?: 'home' | 'profile' } = $props();
</script>

{#if variant === 'home'}
  <SkeletonRegion class="hs-wrap">
    <section class="hero on-stage">
      <div class="content">
        <div class="idrow">
          <Skeleton variant="box" w={96} h={96} notch={14} class="icn" />
          <div class="idtext">
            <Skeleton variant="bar" lh={17} h={11} w={170} />
            <span class="h1"><Skeleton variant="bar" h="78%" w="clamp(220px, 34vw, 520px)" radius={6} /></span>
            <Skeleton variant="bar" lh={22} h={13} w={300} />
          </div>
        </div>
        <div class="actions">
          <Skeleton variant="box" w={188} h={60} notch={8} />
          <Skeleton variant="bar" lh={36} h={13} w={110} />
        </div>
      </div>
      <div class="edge step-edge"></div>
    </section>
  </SkeletonRegion>
{:else}
  <SkeletonRegion class="hs-wrap">
    <section class="phero on-stage">
      <div class="inner">
        <Skeleton variant="bar" lh={28} h={12} w={76} />
        <div class="row">
          <Skeleton variant="box" w={84} h={84} notch={13} />
          <div class="id">
            <Skeleton variant="bar" lh={42} h={30} w="min(340px, 60%)" radius={6} />
            <div class="chips">{#each [64, 88, 76, 70, 86] as w, i (i)}<Skeleton variant="box" w={w} h={24} radius={4} />{/each}</div>
          </div>
          <Skeleton variant="box" w={188} h={60} notch={8} />
        </div>
      </div>
    </section>
  </SkeletonRegion>
{/if}

<style>
  .hero { position: relative; min-height: 46vh; display: flex; align-items: flex-end; background: var(--hero-bg); }
  .content { position: relative; display: grid; gap: 14px; justify-items: start; padding: 40px 40px 44px 44px; width: 100%; }
  .idrow { display: flex; align-items: flex-end; gap: 20px; min-width: 0; }
  .idrow :global(.icn) { margin-bottom: 4px; }
  .idtext { display: grid; gap: 14px; justify-items: start; min-width: 0; }
  .h1 { display: flex; align-items: center; height: calc(var(--fs-hero) * 0.95); }
  .actions { display: flex; gap: 8px 16px; align-items: center; margin-top: 6px; }
  .edge { position: absolute; left: 0; right: 0; bottom: -1px; background: var(--ink-0); }
  .phero { position: relative; min-height: 190px; border-radius: var(--r-l); overflow: hidden; border: 1px solid var(--line); display: flex; background: var(--hero-bg); }
  .inner { width: 100%; display: flex; flex-direction: column; justify-content: space-between; padding: 10px 24px 18px 28px; gap: 12px; }
  .row { display: flex; align-items: flex-end; gap: 16px; flex-wrap: wrap; }
  .id { display: grid; gap: 10px; flex: 1; min-width: 0; }
  .chips { display: flex; gap: 8px; }
</style>
