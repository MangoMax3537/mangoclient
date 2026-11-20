<script lang="ts">
  import PlusIcon from 'phosphor-svelte/lib/PlusIcon';
  import PlayIcon from 'phosphor-svelte/lib/PlayIcon';
  import ArrowRightIcon from 'phosphor-svelte/lib/ArrowRightIcon';
  import SlidersIcon from 'phosphor-svelte/lib/SlidersIcon';
  import PencilSimpleIcon from 'phosphor-svelte/lib/PencilSimpleIcon';
  import ImageIcon from 'phosphor-svelte/lib/ImageIcon';
  import InstanceIcon from './InstanceIcon.svelte';
  import PlayButton from './PlayButton.svelte';
  import LaunchProgress from './LaunchProgress.svelte';
  import Console from './Console.svelte';
  import ProfileMenu from './ProfileMenu.svelte';
  import QuickJoin from './QuickJoin.svelte';
  import HomeBackground from './HomeBackground.svelte';
  import Spark from './Spark.svelte';
  import EmptyState from './EmptyState.svelte';
  import NoAccountHint from './NoAccountHint.svelte';
  import { fmtDuration, fmtRam, fmtRelative } from '../lib/format';
  import type { Key } from '../lib/i18n';
  import { tip } from '../lib/tooltip';
  import { cardHandlers } from '../lib/instanceCard';
  import { instanceMenu } from '../lib/instanceMenu';
  import { contextmenu } from '../lib/contextMenu.svelte';
  import {
    LAUNCH_STAGES, actions, app, consoles, getApi, isBusy, isRunning, lang, launchInfo, nav, openBackgroundPicker, openIconPicker, openProfile, selectedProfile, stageProgress, t, tp,
  } from '../lib/stores.svelte';
  import type { StatsSummary } from '../lib/types';
  import Loadable from './Loadable.svelte';
  import Skeleton from './skeleton/Skeleton.svelte';
  import SkeletonRegion from './skeleton/SkeletonRegion.svelte';
  import { createLoadState } from '../lib/loadState.svelte';

  const p = $derived(selectedProfile());
  const info = $derived(p ? launchInfo(p.id) : null);
  const busy = $derived(p ? isBusy(p.id) : false);
  const running = $derived(p ? isRunning(p.id) : false);
  const launching = $derived(busy && !running);
  const ram = $derived(p ? p.effectiveRamMb : 0);
  const loaderLabel = $derived(p ? (p.loader === 'vanilla' ? t('profile.vanilla') : `${p.loader} ${p.loaderVersion ?? ''}`.trim()) : '');
  const showConsole = $derived(p ? busy || (consoles[p.id]?.length ?? 0) > 0 : false);
  const others = $derived([...app.profiles].sort((a, b) => (b.lastPlayed ?? 0) - (a.lastPlayed ?? 0)).slice(0, 12));
  const pct = $derived(info ? Math.round(stageProgress(info) * 100) : 0);
  const stageLabel = $derived(info && launching ? t(`launch.${info.state}` as Key) : '');
  const label = $derived(running ? t('home.stop') : launching ? `${stageLabel}${info?.state === 'downloading' ? ` ${pct}%` : ''}` : t('home.play'));
  const prog = $derived(launching && info ? (info.state === 'downloading' ? stageProgress(info) : null) : null);
  void LAUNCH_STAGES;

  const stats = createLoadState<StatsSummary>({ isEmpty: () => false });
  $effect(() => { stats.run(() => getApi().statsSummary({ days: 14 })); return () => stats.cancel(); });
  const sparkLabel = (st: StatsSummary) => t('stats.chartLabel', { days: 14, total: fmtDuration(st.totalMs) });

  const ops = { select: (id: string) => void actions.selectProfile(id), open: (id: string) => openProfile(id), isSelected: (id: string) => selectedProfile()?.id === id };
  function toggle() {
    if (!p) return;
    if (busy) actions.stop(p.id);
    else actions.play(p.id);
  }
</script>

<div class="home">
  {#if p && info}
    <section class="hero on-stage" aria-labelledby="hero-h" use:contextmenu={() => instanceMenu(p)}>
      {#key p.id}<HomeBackground profileId={p.id} mcVersion={p.mcVersion} background={p.background} tint={p.color} />{/key}
      <button class="bgbtn btn" type="button" use:tip={{ text: t('bg.change'), side: 'top' }} aria-label={t('bg.change')} onclick={() => openBackgroundPicker(p.id)}><ImageIcon weight="bold" size={16} aria-hidden="true" /></button>
      <div class="content">
        <div class="idrow">
          <button class="iconbtn" type="button" use:tip={{ text: t('icons.change'), side: 'top' }} aria-label={t('icons.change')} onclick={() => openIconPicker(p.id)}>
            <InstanceIcon profile={p} size={96} notch={14} />
            <span class="pen" aria-hidden="true"><PencilSimpleIcon weight="bold" size={14} /></span>
          </button>
          <div class="idtext">
        <div class="eyebrow status">
          {#if running}<span class="sq ok live"></span><span class="live-t">{t('home.running')}</span>
          {:else if launching}<span class="sq warn live"></span>{t(`launch.${info.state}` as Key)}
          {:else}{t('home.lastPlayed')}<span class="sep">/</span>{#if p.lastPlayed}{fmtRelative(p.lastPlayed, lang())}{:else}{t('home.never')}{/if}{/if}
        </div>
        <h1 id="hero-h" class="display">{p.name}</h1>
        <ul class="facts" aria-label={p.name}>
          <li>{p.mcVersion}</li>
          <li>{loaderLabel}</li>
          <li>{t('chip.java')} {t('chip.auto')}</li>
          <li>{ram ? fmtRam(ram) : t('chip.default')}</li>
          {#if p.modCount}<li>{tp('profile.mods', p.modCount)}</li>{/if}
        </ul>
          </div>
        </div>
        <div class="actions">
          <PlayButton mode={running ? 'stop' : launching ? 'busy' : 'play'} {label} progress={prog} onclick={toggle} disabled={!app.accounts.length && !busy} />
          {#if launching}<button class="btn text" type="button" onclick={toggle}>{t('home.stop')}</button>{/if}
          {#if !app.accounts.length && !busy}<NoAccountHint />{/if}
          <ProfileMenu currentId={p.id} disabled={busy} />
          <button class="btn text" type="button" onclick={() => openProfile(p.id)}><SlidersIcon weight="bold" size={16} aria-hidden="true" />{t('home.openProfile')}</button>
        </div>
        {#if launching}<LaunchProgress profileId={p.id} />{/if}
      </div>
      <div class="total" aria-hidden="true">
        <span class="n display">{fmtDuration(p.playTimeMs)}</span>
        <span class="eyebrow">{t('home.playtime')}</span>
      </div>
    </section>

    {#if showConsole}<div class="pad"><Console profileId={p.id} /></div>{/if}

    <div class="body">
      <section aria-labelledby="jb-h">
        <header>
          <h2 id="jb-h">{t('home.jumpBack')}</h2>
          <button class="btn text" type="button" onclick={() => (nav.view = 'library')}>{t('home.continue')}<ArrowRightIcon weight="bold" size={14} aria-hidden="true" /></button>
        </header>
        <ul class="strip">
          {#each others as o (o.id)}
            {@const ob = isBusy(o.id)}
            <li class="card notch" class:sel={o.id === p.id} class:live={ob} style:--n="14px" use:contextmenu={() => instanceMenu(o)}>
              <button class="open" type="button" aria-pressed={o.id === p.id} {...cardHandlers(o.id, ops)}>
                <span class="art"><InstanceIcon profile={o} fill radius={0} /></span>
                <span class="nm">{o.name}</span>
                <span class="meta mono">{o.mcVersion}<span class="sep">/</span>{o.loader === 'vanilla' ? t('profile.vanilla') : o.loader}</span>
                <span class="meta mono dim">{fmtDuration(o.playTimeMs)}<span class="sep">/</span>{fmtRelative(o.lastPlayed, lang()) ?? t('home.never')}</span>
              </button>
              <button class="go notch" style:--n="8px" type="button" disabled={ob || !app.accounts.length} use:tip={{ text: `${t('home.play')}: ${o.name}`, side: 'top' }} aria-label="{t('home.play')}: {o.name}"
                onclick={async () => { await actions.selectProfile(o.id); actions.play(o.id); }}>
                <PlayIcon weight="bold" size={16} aria-hidden="true" />
              </button>
            </li>
          {/each}
        </ul>
      </section>

      <div class="split">
        <QuickJoin />
        {#if stats.phase !== 'ready' || (stats.data?.totalMs ?? 0) > 0}
        <section class="pt" aria-labelledby="pt-h">
          <header><h2 id="pt-h">{t('home.last14')}</h2></header>
          <Loadable state={stats} compactError>
            {#snippet skeleton()}
              <SkeletonRegion class="spk">
                <div class="bigsk"><Skeleton variant="bar" h={34} w={130} radius={6} /></div>
                <Skeleton variant="box" w="100%" h={64} radius={4} />
                <div class="axissk"><Skeleton variant="bar" lh={17} h={9} w={16} /><Skeleton variant="bar" lh={17} h={9} w={10} /></div>
              </SkeletonRegion>
            {/snippet}
            {#snippet children(st)}
              <div class="big display">{fmtDuration(st.totalMs)}</div>
              <Spark values={st.days.map((d) => d.ms)} label={sparkLabel(st)} />
              <div class="axis mono" aria-hidden="true"><span>14</span><span>0</span></div>
            {/snippet}
          </Loadable>
        </section>
        {/if}
      </div>
    </div>
  {:else}
    <div class="pad">
      <EmptyState title={t('home.noProfiles')} hint={t('home.noProfilesHint')}>
        <button class="btn accent" type="button" onclick={() => (nav.newProfile = true)}><PlusIcon weight="bold" size={16} aria-hidden="true" />{t('library.new')}</button>
      </EmptyState>
    </div>
  {/if}
</div>

<style>
  .home { height: 100%; overflow: auto; display: flex; flex-direction: column; }
  /* hero: full bleed to the content edge, ~46% of the viewport */
  .hero { position: relative; flex: none; min-height: 46vh; display: flex; align-items: flex-end; background: var(--hero-bg); }
  .content { position: relative; display: grid; gap: 14px; justify-items: start; padding: 40px 40px 44px 44px; width: 100%; }
  .status { display: inline-flex; align-items: center; gap: 8px; color: var(--paper-dim); }
  .idrow { display: flex; align-items: flex-end; gap: 20px; min-width: 0; }
  .idtext { display: grid; gap: 14px; justify-items: start; min-width: 0; }
  .iconbtn { position: relative; flex: none; display: block; width: 96px; height: 96px; margin-bottom: 4px; }
  .iconbtn:focus-visible { outline: 2px solid var(--focus); outline-offset: 4px; }
  .pen { position: absolute; right: 4px; bottom: 4px; width: 24px; height: 24px; display: grid; place-items: center; background: var(--accent-fill); color: var(--on-fill); opacity: 0; transform: translateY(3px); transition: opacity var(--dur) var(--ease), transform var(--dur) var(--ease); }
  .iconbtn:hover .pen, .iconbtn:focus-visible .pen { opacity: 1; transform: none; }
  .bgbtn { position: absolute; right: 16px; top: 14px; width: 36px; padding: 0; background: rgba(var(--scrim), 0.55); border-color: var(--hero-line); color: var(--paper); z-index: 2; }
  .sep { margin: 0 8px; opacity: 0.5; }
  .live-t { color: var(--leaf); }
  .live { animation: pulse 1.6s ease-in-out infinite; }
  h1 { font-size: var(--fs-hero); max-width: 14ch; text-wrap: balance; color: var(--paper); }
  .facts { color: var(--paper-dim); }
  .actions { display: flex; flex-wrap: wrap; gap: 8px 16px; align-items: center; margin-top: 6px; }
  .total { position: absolute; right: 40px; bottom: 44px; display: grid; justify-items: end; gap: 4px; }
  .total .n { font-size: var(--fs-2xl); color: var(--paper); letter-spacing: -0.02em; font-feature-settings: 'tnum'; }
  .pad { padding: var(--s-4) 32px 0; }

  .body { display: grid; grid-template-columns: minmax(0, 1fr); gap: var(--s-6); padding: var(--s-5) 32px 32px 44px; }
  header { display: flex; justify-content: space-between; align-items: baseline; margin-bottom: var(--s-3); }
  h2 { font-size: var(--fs-xl); }
  .strip { list-style: none; margin: 0 -32px 0 -4px; padding: 4px 32px 8px 4px; display: flex; gap: var(--s-3); overflow-x: auto; scroll-snap-type: x proximity; scrollbar-width: thin; }
  .card { position: relative; flex: none; width: 212px; scroll-snap-align: start; background: var(--ink-2); transition: transform var(--dur) var(--ease); }
  .card:hover { transform: translateY(-2px); }
  .card.sel { background: var(--ink-3); }
  .open { display: grid; width: 100%; text-align: left; gap: 2px; padding-bottom: 12px; }
  .open:focus-visible { outline: 2px solid var(--focus); outline-offset: -4px; }
  .art { display: block; aspect-ratio: 16 / 10; margin-bottom: 10px; }
  .nm { font: 700 var(--fs-lg)/1.2 var(--font-display); padding: 0 12px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .meta { padding: 0 12px; font-size: var(--fs-sm); color: var(--paper-dim); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .meta.dim { color: var(--paper-mute); }
  .meta .sep { margin: 0 6px; }
  .card.live .nm::after { content: ''; display: inline-block; width: 7px; height: 7px; margin-left: 8px; background: var(--leaf); animation: pulse 1.6s ease-in-out infinite; }
  .go { position: absolute; right: 8px; top: 8px; width: 32px; height: 32px; display: grid; place-items: center; background: var(--accent-fill); color: var(--on-fill); opacity: 0; transform: translateY(-4px); transition: opacity var(--dur) var(--ease), transform var(--dur) var(--ease); z-index: 2; }
  .card:hover .go, .go:focus-visible, .card:focus-within .go { opacity: 1; transform: none; }
  .go:disabled { display: none; }

  .split:empty { display: none; }
  .split { display: grid; grid-template-columns: minmax(0, 1fr) minmax(0, 1fr); gap: var(--s-7); align-items: start; }
  .pt { min-width: 0; display: grid; gap: var(--s-2); }
  .pt header { margin-bottom: 0; }
  .big { font-size: var(--fs-3xl); color: var(--mango); line-height: 1; margin: 4px 0 var(--s-2); }
  :global(.spk) { display: grid; gap: var(--s-2); }
  .bigsk { display: flex; align-items: center; height: 40px; margin: 4px 0 var(--s-2); }
  .axissk { display: flex; justify-content: space-between; }
  .axis { display: flex; justify-content: space-between; font-size: var(--fs-xs); color: var(--paper-mute); }
  @media (max-width: 1100px) { .split { grid-template-columns: 1fr; gap: var(--s-6); } .total { display: none; } }
  @keyframes pulse { 50% { opacity: 0.4; } }
</style>
