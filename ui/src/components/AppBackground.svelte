<script lang="ts">
  // The Home hero background. A fixed choice (image, wallpaper, panorama, none) is drawn as is. With `auto` the hero crossfades through the
  // background pool (generic wallpapers + one panorama per installed version + the brand surface) every 30..60 s in shuffled order:
  // exactly two stacked layers, the next one is loaded AND decoded before it fades in (opacity only, ~1.8 s), then the old layer is dropped
  // so its images leave memory. The timer pauses while the document is hidden or the window unfocused; reduced motion = no cycling.
  import { onMount } from 'svelte';
  import BgLayer from './BgLayer.svelte';
  import PanoramaBackground from './PanoramaBackground.svelte';
  import { backgroundStyle, panoramaAngle, resolveBackground, resolvePoolItem, type ResolvedBackground } from '../lib/instanceIcon';
  import { CycleScheduler, FADE_MS, Order, buildPool, poolKey, type PoolItem } from '../lib/bgCycle';
  import { reducedMotion } from '../lib/motion';
  import { wallpapers } from '../lib/wallpapers';
  import { loadPanorama, panoramaPool, panoramas, ui } from '../lib/stores.svelte';
  import type { ProfileBackground } from '../lib/types';

  interface Props { profileId: string; mcVersion: string; background: ProfileBackground; tint?: string | null }
  let { profileId, mcVersion, background, tint = null }: Props = $props();

  const isAuto = $derived(background.kind === 'auto');
  $effect(() => { if (isAuto) void loadPanorama(profileId); });
  const pool = $derived(buildPool(wallpapers, panoramaPool.sets));
  const calm = reducedMotion();
  const cycling = $derived(isAuto && ui.cycleBackgrounds && !calm && pool.length > 1);

  /** `?mockCycleMs=3000` (dev) shortens the 30..60 s interval. */
  const fixedMs = (() => { try { const n = Number(new URLSearchParams(location.search).get('mockCycleMs')); return Number.isFinite(n) && n > 0 ? n : null; } catch { return null; } })();

  interface Slot { res: ResolvedBackground | null; key: string; shown: boolean }
  let slots = $state<[Slot, Slot]>([{ res: null, key: '', shown: false }, { res: null, key: '', shown: false }]);
  let base = $state<0 | 1>(0);          // the settled layer
  let incoming = $state<0 | 1 | null>(null);
  let fading = false;

  const order = new Order();
  let started = false;
  let timer: CycleScheduler | null = null;
  let fadeTimer: ReturnType<typeof setTimeout> | undefined;
  const isPaused = (): boolean => document.hidden || !document.hasFocus();

  // the first layer: what the instance itself shows (same resolution as its own hero)
  const first = $derived(resolveBackground(background, { mcVersion, profileId, panorama: panoramas[profileId], wallpapers, pool }));
  $effect(() => {
    if (!cycling) return;
    if (!started) {
      started = true;
      slots[0] = { res: first, key: 'first', shown: true };
    } else if (incoming === null && slots[base].key === 'first') {
      slots[base].res = first; // the own panorama arrived after the first paint: keep the base in step until the first swap
    }
  });
  $effect(() => { order.setPool(pool); });

  /** The pool key of what a resolved background shows, so the shuffle never starts with the same one. */
  const keyOf = (r: ResolvedBackground): string | null =>
    r.kind === 'image' ? (pool.find((p) => p.kind === 'wallpaper' && p.url === r.url) ? poolKey(pool.find((p) => p.kind === 'wallpaper' && p.url === r.url) as PoolItem) : null)
    : r.kind === 'panorama' ? `p:${r.set.mcVersion}` : r.kind === 'watermark' ? 'brand' : null;
  function advance(): void {
    order.seed(slots[base].res ? keyOf(slots[base].res as ResolvedBackground) : null);
    const next: PoolItem | null = order.next();
    if (!next) { timer?.done(); return; }
    const other: 0 | 1 = base === 0 ? 1 : 0;
    slots[other] = { res: resolvePoolItem(next), key: poolKey(next), shown: false };
    incoming = other;
  }
  /** The incoming layer has loaded and decoded: fade it in, then drop the old one. */
  function ready(i: 0 | 1): void {
    if (incoming !== i || slots[i].shown || fading) return;
    fading = true;
    // two frames at opacity 0 first, so the fade really transitions (a cached image is ready in the very frame it mounts)
    requestAnimationFrame(() => requestAnimationFrame(() => {
      slots[i].shown = true;
      clearTimeout(fadeTimer);
      fadeTimer = setTimeout(() => {
        const old: 0 | 1 = i === 0 ? 1 : 0;
        base = i; incoming = null; fading = false;
        slots[old] = { res: null, key: '', shown: false };
        timer?.done();
      }, FADE_MS + 60);
    }));
  }

  onMount(() => {
    timer = new CycleScheduler({ setTimeout: (f, ms) => setTimeout(f, ms), clearTimeout: (h) => clearTimeout(h as ReturnType<typeof setTimeout>), fixedMs }, advance);
    const pause = () => timer?.setPaused(isPaused());
    document.addEventListener('visibilitychange', pause);
    window.addEventListener('blur', pause); window.addEventListener('focus', pause);
    pause();
    return () => {
      document.removeEventListener('visibilitychange', pause);
      window.removeEventListener('blur', pause); window.removeEventListener('focus', pause);
      timer?.stop(); clearTimeout(fadeTimer);
    };
  });
  // start / stop with the preference and the pool
  $effect(() => {
    if (!timer) return;
    if (cycling) {
      if (!timer.running) { timer.start(); timer.setPaused(isPaused()); }
    } else if (timer.running) timer.stop();
  });
  const look = (r: ResolvedBackground) => backgroundStyle(background, r);
</script>

{#if !cycling}
  <PanoramaBackground {profileId} {mcVersion} {background} {tint} />
{:else}
  <div class="stack" aria-hidden="true">
    {#each slots as s, i (i)}
      {#if s.res}
        <div class="slot" class:in={incoming === i} class:shown={s.shown}>
          <BgLayer res={s.res} look={look(s.res)} {tint} angle={panoramaAngle(profileId)} paused={false} onready={() => ready(i as 0 | 1)} />
        </div>
      {/if}
    {/each}
  </div>
{/if}

<style>
  .stack { position: absolute; inset: 0; overflow: hidden; background: var(--hero-bg); }
  .slot { position: absolute; inset: 0; }
  .slot.in { z-index: 2; opacity: 0; transition: opacity 1.8s ease-in-out; }
  .slot.in.shown { opacity: 1; }
</style>
