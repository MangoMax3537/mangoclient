<script lang="ts">
  // THE background engine, mounted once in the app shell behind every view. Two stacked layers: the next image is loaded AND decoded before it
  // fades in (opacity only, ~1.8 s), then the old layer is dropped so its images leave memory. mode 'hero' (Home) draws the full-strength hero
  // look (the Home hero is transparent over it); mode 'full' (every other view) draws a still image under a strong uniform scrim and blur.
  // With `auto` (and `cycle`) it crossfades through the pool (generic wallpapers + panoramas + brand surface) every 30..60 s in shuffled order.
  // The timer pauses while the document is hidden or the window unfocused; reduced motion = no cycling.
  import { onMount, untrack } from 'svelte';
  import BgLayer from './BgLayer.svelte';
  import { backgroundStyle, panoramaAngle, resolveBackground, resolvePoolItem, type ResolvedBackground } from '../lib/instanceIcon';
  import { CycleScheduler, FADE_MS, Order, buildPool, poolKey, type PoolItem } from '../lib/bgCycle';
  import { reducedMotion } from '../lib/motion';
  import { wallpapers, DEFAULT_SCRIM } from '../lib/wallpapers';
  import { FULL_BLUR, FULL_DIM, flatten } from '../lib/bgMode';
  import { loadPanorama, panoramaPool, panoramas, ui } from '../lib/stores.svelte';
  import type { ProfileBackground } from '../lib/types';

  interface Props { profileId: string; mcVersion: string; background: ProfileBackground; tint?: string | null; mode?: 'hero' | 'full'; cycle?: boolean }
  let { profileId, mcVersion, background, tint = null, mode = 'hero', cycle = true }: Props = $props();
  void DEFAULT_SCRIM;

  const isAuto = $derived(background.kind === 'auto');
  $effect(() => { if (profileId && (isAuto || background.kind === 'panorama')) void loadPanorama(profileId); });
  const pool = $derived(buildPool(wallpapers, panoramaPool.sets));
  const calm = reducedMotion();
  const cycling = $derived(cycle && isAuto && ui.cycleBackgrounds && !calm && pool.length > 1);

  /** `?mockCycleMs=3000` (dev) shortens the 30..60 s interval. */
  const fixedMs = (() => { try { const n = Number(new URLSearchParams(location.search).get('mockCycleMs')); return Number.isFinite(n) && n > 0 ? n : null; } catch { return null; } })();

  interface Slot { res: ResolvedBackground | null; key: string; shown: boolean }
  let slots = $state<[Slot, Slot]>([{ res: null, key: '', shown: false }, { res: null, key: '', shown: false }]);
  let base = $state<0 | 1>(0);          // the settled layer
  let incoming = $state<0 | 1 | null>(null);
  let fading = false;

  const order = new Order();
  let timer: CycleScheduler | null = null;
  let fadeTimer: ReturnType<typeof setTimeout> | undefined;
  let watchdog: ReturnType<typeof setTimeout> | undefined;
  const isPaused = (): boolean => document.hidden || !document.hasFocus();

  // what the instance itself shows (same resolution as its own hero)
  const first = $derived(resolveBackground(background, { mcVersion, profileId, panorama: panoramas[profileId], wallpapers, pool }));
  /** `first` is applied while nothing has been swapped in yet (a changed setting or a late panorama updates it in place). */
  $effect(() => {
    const f = first;
    untrack(() => {
      if (slots[base].res === null || slots[base].key === 'first') {
        slots[base] = { res: f, key: 'first', shown: true };
        if (incoming !== null) { slots[incoming === 0 ? 0 : 1] = { res: null, key: '', shown: false }; incoming = null; fading = false; clearTimeout(fadeTimer); timer?.done(); }
      }
    });
  });
  // the setting itself changed (kind / image): forget the cycled state
  let settingKey = '';
  $effect(() => {
    const k = `${profileId}|${JSON.stringify(background)}`;
    untrack(() => {
      if (settingKey && k !== settingKey && slots[base].key !== 'first') {
        const other: 0 | 1 = base === 0 ? 1 : 0;
        slots[other] = { res: null, key: '', shown: false }; incoming = null; fading = false; clearTimeout(fadeTimer);
        slots[base] = { res: first, key: 'first', shown: true };
        timer?.done();
      }
      settingKey = k;
    });
  });
  $effect(() => { order.setPool(pool); });

  /** The pool key of what a resolved background shows, so the shuffle never starts with the same one. */
  const keyOf = (r: ResolvedBackground): string | null => {
    if (r.kind === 'image') { const m = pool.find((p) => p.kind === 'wallpaper' && p.url === r.url); return m ? poolKey(m) : null; }
    return r.kind === 'panorama' ? `p:${r.set.mcVersion}` : r.kind === 'watermark' ? 'brand' : null;
  };
  function advance(): void {
    const cur = slots[base].res;
    order.seed(cur ? keyOf(cur) : null);
    const next: PoolItem | null = order.next();
    if (!next || incoming !== null) { timer?.done(); return; }
    const other: 0 | 1 = base === 0 ? 1 : 0;
    slots[other] = { res: resolvePoolItem(next), key: poolKey(next), shown: false };
    incoming = other;
    // watchdog: an image that never loads must not stall the cycle for good
    clearTimeout(watchdog);
    watchdog = setTimeout(() => {
      if (incoming === null || fading) return;
      slots[incoming] = { res: null, key: '', shown: false }; incoming = null;
      timer?.done();
    }, 20_000);
  }
  /** The incoming layer has loaded and decoded: fade it in, then drop the old one. */
  function ready(i: 0 | 1): void {
    if (incoming !== i || slots[i].shown || fading) return;
    fading = true;
    clearTimeout(watchdog);
    // two frames at opacity 0 first, so the fade really transitions; a timeout backs it up (rAF does not run in a hidden window)
    let went = false;
    const go = () => {
      if (went) return; went = true;
      slots[i].shown = true;
      clearTimeout(fadeTimer);
      fadeTimer = setTimeout(() => {
        const old: 0 | 1 = i === 0 ? 1 : 0;
        base = i; incoming = null; fading = false;
        slots[old] = { res: null, key: '', shown: false };
        timer?.done();
      }, FADE_MS + 60);
    };
    requestAnimationFrame(() => requestAnimationFrame(go));
    setTimeout(go, 250);
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
      timer?.stop(); clearTimeout(fadeTimer); clearTimeout(watchdog);
    };
  });
  // start / stop with the preference and the pool
  $effect(() => {
    if (!timer) return;
    if (cycling) {
      if (!timer.running) { timer.start(); timer.setPaused(isPaused()); }
    } else if (timer.running) timer.stop();
  });
  const full = $derived(mode === 'full');
  const shown = (r: ResolvedBackground): ResolvedBackground => (full ? flatten(r) : r);
  const look = (r: ResolvedBackground) => (full ? { dim: FULL_DIM, blur: FULL_BLUR } : backgroundStyle(background, r));
</script>

<div class="stack" class:full aria-hidden="true" data-bg-engine>
  {#each slots as s, i (i)}
    {#if s.res}
      {@const r = shown(s.res)}
      <div class="slot" class:in={incoming === i} class:shown={s.shown}>
        <BgLayer res={r} look={look(r)} {tint} {full} angle={panoramaAngle(profileId)} paused={false} onready={() => ready(i as 0 | 1)} />
      </div>
    {/if}
  {/each}
</div>

<style>
  /* its own stacking context: the crossfading layer (z-index 2) can never climb over the content above the engine */
  .stack { position: absolute; inset: 0; overflow: hidden; background: var(--hero-bg); isolation: isolate; z-index: 0; pointer-events: none; }
  .stack.full { background: var(--ink-0); }
  .slot { position: absolute; inset: 0; }
  .slot.in { z-index: 2; opacity: 0; transition: opacity 1.8s ease-in-out; }
  .slot.in.shown { opacity: 1; }
</style>
