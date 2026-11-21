<script lang="ts">
  // THE background engine, mounted once in the app shell behind every view. Two stacked layers: the next image is loaded AND decoded before it
  // fades in (opacity only, ~1.8 s), then the old layer is dropped so its images leave memory. mode 'hero' (Home) draws the full-strength hero
  // look (the Home hero is transparent over it); mode 'full' (every other view) draws a still image under a strong uniform scrim and blur.
  // With `auto` (and `cycle`) it crossfades through the pool (version wallpaper + generic wallpapers, all bundled with the app) every 30 s in shuffled order.
  // The timer pauses while the document is hidden or the window was blurred. Reduced motion does NOT stop the cycle (WebKitGTK reports it whenever the desktop has animations off): the swap is just instant. Only the Settings toggle turns cycling off.
  import { onMount, untrack } from 'svelte';
  import BgLayer from './BgLayer.svelte';
  import { backgroundStyle, panoramaAngle, resolveBackground, resolvePoolItem, type ResolvedBackground } from '../lib/instanceIcon';
  import { CycleScheduler, FADE_MS, Order, buildPool, poolKey, versionWallpaper, type PoolItem } from '../lib/bgCycle';
  import { reducedMotion } from '../lib/motion';
  import { wallpapers, DEFAULT_SCRIM } from '../lib/wallpapers';
  import { FULL_BLUR, FULL_DIM, flatten } from '../lib/bgMode';
  import { artUrl, floorCandidates, readLastWallpaper, rememberWallpaper } from '../lib/bgFloor';
  import { loadPanorama, panoramas, ui } from '../lib/stores.svelte';
  import type { ProfileBackground } from '../lib/types';
  import { bgNow } from '../lib/bgNow.svelte';

  interface Props {
    profileId: string; mcVersion: string; background: ProfileBackground; tint?: string | null; mode?: 'hero' | 'full'; cycle?: boolean;
    /** Follow the main engine's slide (bgNow) instead of cycling on its own: the picker's Auto preview. */
    mirror?: boolean;
  }
  let { profileId, mcVersion, background, tint = null, mode = 'hero', cycle = true, mirror = false }: Props = $props();
  void DEFAULT_SCRIM;

  const isAuto = $derived(background.kind === 'auto');
  $effect(() => { if (profileId && background.kind === 'panorama') void loadPanorama(profileId); });
  // slides that failed to load or decode this session: skipped from now on, the current slide stays
  let bad = $state<string[]>([]);
  const isBad = (r: ResolvedBackground | null): boolean => { const u = r && artUrl(r); return !!u && bad.includes(u); };
  const pool = $derived(buildPool(wallpapers, versionWallpaper(wallpapers, mcVersion, profileId)).filter((p) => !isBad(resolvePoolItem(p))));
  // THE floor: a bundled wallpaper under everything (the main engine picks a random generic one per app start; a mirror keeps the
  // last shown one). Drawn at once, never faded, never removed. If the floor image itself fails to load, the next bundled wallpaper takes
  // over (loudly, in the console): the window is never left without an image.
  const pickAny = <T,>(l: readonly T[]): T | null => (l.length ? l[Math.floor(Math.random() * l.length)] : null);
  const floorList = floorCandidates(wallpapers, untrack(() => mirror) ? readLastWallpaper() : (pickAny(wallpapers.filter((w) => w.generic))?.id ?? readLastWallpaper()));
  let floorIdx = $state(0);
  const floor = $derived(floorList[floorIdx] ?? null);
  const floorRes = $derived<ResolvedBackground | null>(floor ? { kind: 'image', url: floor.url, scrim: floor.scrim, source: 'wallpaper' } : null);
  function floorFailed(): void {
    console.warn('[bg] floor wallpaper failed to load, trying the next bundled one:', floor?.url);
    if (floorIdx < floorList.length - 1) floorIdx += 1;
  }
  /** Only real images and panoramas become slides; plain / watermark results leave the floor showing. */
  const real = (r: ResolvedBackground | null): ResolvedBackground | null => (r && (r.kind === 'image' || r.kind === 'panorama') ? r : null);
  function remember(r: ResolvedBackground): void {
    if (r.kind !== 'image') return;
    const w = wallpapers.find((x) => x.url === r.url);
    if (w) rememberWallpaper(w.id);
  }
  const calm = reducedMotion();
  const cycling = $derived(!mirror && cycle && isAuto && ui.cycleBackgrounds && !!profileId && pool.length > 1);
  // dev-only: why the cycle is off (console, never UI)
  $effect(() => { if (import.meta.env.DEV && !cycling) console.debug('[bg] cycling off:', { cycle, isAuto, setting: ui.cycleBackgrounds, profileId, pool: pool.length }); });

  /** `?mockCycleMs=3000` (dev) shortens the 30 s interval. */
  const fixedMs = (() => { try { const n = Number(new URLSearchParams(location.search).get('mockCycleMs')); return Number.isFinite(n) && n > 0 ? n : null; } catch { return null; } })();

  interface Slot { res: ResolvedBackground | null; key: string; shown: boolean }
  let slots = $state<[Slot, Slot]>([{ res: null, key: '', shown: false }, { res: null, key: '', shown: false }]);
  let base = $state<0 | 1>(0);          // the settled layer
  let incoming = $state<0 | 1 | null>(null);
  let fading = false;

  const order = new Order();
  // reactive on purpose: the start/stop effect below must re-run once the scheduler exists, whatever the effect order
  let timer = $state.raw<CycleScheduler | null>(null);
  let blurred = false;
  let fadeTimer: ReturnType<typeof setTimeout> | undefined;
  let watchdog: ReturnType<typeof setTimeout> | undefined;
  // document.hasFocus() is unreliable in WebKitGTK/Wayland (false while focused), so only real blur events count
  const isPaused = (): boolean => document.hidden || blurred;

  // what the instance itself shows (same resolution as its own hero)
  const own = $derived(resolveBackground(background, { mcVersion, profileId, panorama: panoramas[profileId], wallpapers, pool }));
  // a mirror starts on what the main engine shows (read once: later changes fade in through the follow effect below)
  const first = $derived(mirror && isAuto ? (untrack(() => bgNow.res) ?? own) : own);
  /** The main engine's `auto` start: a random member of the instance's pool (version wallpaper + generic wallpapers), new on every app start. */
  const randomStart = (): ResolvedBackground | null => { const i = pickAny(pool); return i ? real(resolvePoolItem(i)) : null; };
  const autoStart = $derived(isAuto && !mirror);
  /** `first` is applied while nothing has been swapped in yet (a changed setting or a late panorama updates it in place). */
  $effect(() => {
    const f0 = real(first);
    const pid = profileId;
    void pool;
    untrack(() => {
      // auto before the instance is known (boot): the floor shows, the random start comes with the instance's pool
      if (autoStart && !pid) return;
      // a slide that failed before is not tried again: the next good pool member (or the floor) takes its place
      const f = autoStart ? randomStart() : isBad(f0) ? (pool.length ? real(resolvePoolItem(pool[0])) : null) : f0;
      // while `auto` the engine is one continuous show: switching view / instance never swaps the slide in place (only the first fill)
      if (slots[base].res === null || (slots[base].key === 'first' && !(isAuto && settingKey === 'auto|'))) {
        slots[base] = { res: f, key: 'first', shown: true };
        if (incoming !== null) { slots[incoming === 0 ? 0 : 1] = { res: null, key: '', shown: false }; incoming = null; fading = false; clearTimeout(fadeTimer); timer?.done(); }
      }
    });
  });
  // the setting itself changed (kind / image): forget the cycled state. Another `auto` instance selected: fade to a new random slide.
  let settingKey = '';
  let lastProfile = '';
  $effect(() => {
    const k = background.kind === 'auto' ? 'auto|' : `${profileId}|${JSON.stringify(background)}`;
    const pid = profileId;
    untrack(() => {
      const switched = !!lastProfile && !!pid && pid !== lastProfile;
      if (settingKey && k !== settingKey && slots[base].key !== 'first') {
        const other: 0 | 1 = base === 0 ? 1 : 0;
        slots[other] = { res: null, key: '', shown: false }; incoming = null; fading = false; clearTimeout(fadeTimer);
        slots[base] = { res: autoStart ? randomStart() : real(first), key: 'first', shown: true };
        timer?.done();
      } else if (switched && autoStart && k === settingKey && slots[base].res) switchNow();
      settingKey = k;
      if (pid) lastProfile = pid;
    });
  });
  /** Instance switch while `auto`: crossfade to the next shuffled slide of the new instance's pool and restart the 30-60 s wait. */
  function switchNow(): void {
    if (incoming !== null) return;
    order.setPool(pool);
    const run = !!timer?.running;
    timer?.stop();
    advance();
    if (run) timer?.start();
  }
  $effect(() => { order.setPool(pool); });
  // the main engine publishes what is on screen, or what it is fading to, so a mirror loads it at the same time
  $effect(() => { if (!mirror) bgNow.res = slots[incoming ?? base].res; });
  // a mirror fades to whatever the main engine shows next
  $effect(() => {
    if (!mirror || !isAuto) return;
    const want = real(bgNow.res);
    untrack(() => {
      const u = want && artUrl(want);
      if (!want || !u || incoming !== null || isBad(want)) return;
      const cur = slots[base].res;
      if (cur && artUrl(cur) === u) return;
      const other: 0 | 1 = base === 0 ? 1 : 0;
      slots[other] = { res: want, key: `m:${u}`, shown: false };
      incoming = other;
      clearTimeout(watchdog);
      watchdog = setTimeout(() => { if (incoming !== null && !fading) failed(incoming); }, 20_000);
    });
  });

  /** The pool key of what a resolved background shows, so the shuffle never starts with the same one. */
  const keyOf = (r: ResolvedBackground): string | null => {
    if (r.kind === 'image') { const m = pool.find((p) => p.kind === 'wallpaper' && p.url === r.url); return m ? poolKey(m) : null; }
    return r.kind === 'panorama' ? `p:${r.set.mcVersion}` : null;
  };
  function advance(): void {
    const cur = slots[base].res;
    order.seed(cur ? keyOf(cur) : null);
    const next: PoolItem | null = order.next();
    if (!next || incoming !== null) { timer?.done(); return; }
    const other: 0 | 1 = base === 0 ? 1 : 0;
    const nres = real(resolvePoolItem(next));
    if (!nres) { timer?.done(); return; }
    slots[other] = { res: nres, key: poolKey(next), shown: false };
    incoming = other;
    // watchdog: an image that never loads must not stall the cycle for good
    clearTimeout(watchdog);
    watchdog = setTimeout(() => { if (incoming !== null && !fading) failed(incoming); }, 20_000);
  }
  /** A layer could not load or decode: keep what is on screen, drop the layer, never try that image again this session. */
  function failed(i: 0 | 1): void {
    const u = slots[i].res && artUrl(slots[i].res as ResolvedBackground);
    if (u && !bad.includes(u)) bad = [...bad, u];
    slots[i] = { res: null, key: '', shown: false };
    if (incoming === i) { incoming = null; fading = false; clearTimeout(watchdog); clearTimeout(fadeTimer); timer?.done(); }
  }
  /** The incoming layer has loaded and decoded: fade it in, then drop the old one. */
  function ready(i: 0 | 1): void {
    const cur = slots[i].res;
    if (cur) remember(cur);
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
      }, calm ? 60 : FADE_MS + 60);
    };
    requestAnimationFrame(() => requestAnimationFrame(go));
    setTimeout(go, 250);
  }

  onMount(() => {
    const sched = new CycleScheduler({ setTimeout: (f, ms) => setTimeout(f, ms), clearTimeout: (h) => clearTimeout(h as ReturnType<typeof setTimeout>), fixedMs }, advance);
    timer = sched;
    const pause = () => sched.setPaused(isPaused());
    const onBlur = () => { blurred = true; pause(); };
    const onFocus = () => { blurred = false; pause(); };
    document.addEventListener('visibilitychange', pause);
    window.addEventListener('blur', onBlur); window.addEventListener('focus', onFocus);
    pause();
    return () => {
      document.removeEventListener('visibilitychange', pause);
      window.removeEventListener('blur', onBlur); window.removeEventListener('focus', onFocus);
      sched.stop(); clearTimeout(fadeTimer); clearTimeout(watchdog);
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
  const floorLook = $derived(full ? { dim: FULL_DIM, blur: FULL_BLUR } : { dim: floor?.scrim ?? 0.6, blur: 0 });
  const look = (r: ResolvedBackground) => (full ? { dim: FULL_DIM, blur: FULL_BLUR } : backgroundStyle(background, r));
</script>

<div class="stack" class:full class:calm aria-hidden="true" data-bg-engine data-slide={slots[base].key}>
  {#if floorRes}<div class="floor" data-bg-floor><BgLayer res={floorRes} look={floorLook} {tint} {full} angle={0} instant onbroken={floorFailed} /></div>{/if}
  {#each slots as s, i (i)}
    {#if s.res}
      {@const r = shown(s.res)}
      <div class="slot" class:in={incoming === i} class:shown={s.shown}>
        <BgLayer res={r} look={look(r)} {tint} {full} bare angle={panoramaAngle(profileId)} paused={false} onready={() => ready(i as 0 | 1)} onfail={() => failed(i as 0 | 1)} />
      </div>
    {/if}
  {/each}
</div>

<style>
  /* its own stacking context: the crossfading layer (z-index 2) can never climb over the content above the engine */
  .stack { position: absolute; inset: 0; overflow: hidden; background: var(--hero-bg); isolation: isolate; z-index: 0; pointer-events: none; }
  .stack.full { background: var(--ink-0); }
  .floor, .slot { position: absolute; inset: 0; }
  .slot.in { z-index: 2; opacity: 0; transition: opacity 1.8s ease-in-out; }
  .slot.in.shown { opacity: 1; }
  /* reduced motion: the swap is instant, the cycle itself keeps running */
  .stack.calm .slot.in { transition: none; }
</style>
