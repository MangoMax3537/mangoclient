<script lang="ts">
  import PlusIcon from 'phosphor-svelte/lib/PlusIcon';
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
  import MediaCard from './MediaCard.svelte';
  import MediaStrip from './MediaStrip.svelte';
  import InfoLine from './InfoLine.svelte';
  import Spark from './Spark.svelte';
  import EmptyState from './EmptyState.svelte';
  import NoAccountHint from './NoAccountHint.svelte';
  import { fmtDuration } from '../lib/format';
  import { heroFacts, playedShortFact, versionLoaderFacts } from '../lib/infoLine';
  import type { Key } from '../lib/i18n';
  import { tip } from '../lib/tooltip';
  import { cardHandlers } from '../lib/instanceCard';
  import { instanceMenu } from '../lib/instanceMenu';
  import { contextmenu } from '../lib/contextMenu.svelte';
  import {
    LAUNCH_STAGES, actions, app, consoles, getApi, isBusy, isRunning, lang, launchInfo, nav, openBackgroundPicker, openIconPicker, openProfile, selectedProfile, stageProgress, stopping, t, tp,
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
  const isStopping = $derived(p ? !!stopping[p.id] && busy : false);
  const label = $derived(isStopping ? t('home.stopping') : running ? t('home.stop') : launching ? `${stageLabel}${info?.state === 'downloading' || (info?.state === 'java' && info.progress) ? ` ${pct}%` : ''}` : t('home.play'));
  const prog = $derived(launching && info ? (info.state === 'downloading' || (info.state === 'java' && info.progress) ? stageProgress(info) : null) : null);
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
      <button class="bgbtn btn" type="button" use:tip={{ text: t('bg.change'), side: 'top' }} aria-label={t('bg.change')} onclick={() => openBackgroundPicker(p.id)}><ImageIcon weight="bold" size={16} aria-hidden="true" /></button>
      <div class="content">
        <div class="idrow">
          <button class="iconbtn" type="button" use:tip={{ text: t('icons.change'), side: 'top' }} aria-label={t('icons.change')} onclick={() => openIconPicker(p.id)}>
            <InstanceIcon profile={p} size={96} notch={14} />
            <span class="pen" aria-hidden="true"><PencilSimpleIcon weight="bold" size={14} /></span>
          </button>
          <div class="idtext">
        {#if running || launching}
          <div class="eyebrow status">
            {#if running}<span class="sq ok live"></span><span class="live-t">{t('home.running')}</span>
            {:else}<span class="sq warn live"></span>{t(`launch.${info.state}` as Key)}{/if}
          </div>
        {/if}
        <h1 id="hero-h" class="display">{p.name}</h1>
        <p class="info mono"><InfoLine facts={heroFacts(p, lang(), t)} size={15} /></p>
          </div>
        </div>
        <div class="actions">
          <PlayButton mode={running ? 'stop' : launching ? 'busy' : 'play'} {label} progress={prog} onclick={toggle} disabled={!app.accounts.length && !busy} />
          {#if launching}<button class="btn text" type="button" onclick={toggle} disabled={isStopping}>{isStopping ? t('home.stopping') : t('home.stop')}</button>{/if}
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
        <MediaStrip label={t('home.jumpBack')}>
          {#each others as o (o.id)}
            {@const ob = isBusy(o.id)}
            <li use:contextmenu={() => instanceMenu(o)}>
              <MediaCard title={o.name} selected={o.id === p.id} live={ob} pressed={o.id === p.id} handlers={cardHandlers(o.id, ops)}>
                {#snippet art()}<InstanceIcon profile={o} fill radius={0} />{/snippet}
                {#snippet lines()}
                  <InfoLine facts={versionLoaderFacts(o, t)} />
                  <InfoLine facts={[playedShortFact(o, lang(), t)]} />
                {/snippet}
              </MediaCard>
            </li>
          {/each}
        </MediaStrip>
      </section>

      <QuickJoin />

      <div class="split">
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
        <button class="btn accent" type="button" onclick={() => (nav.newProfile = true)}><PlusIcon weight="bold" size={16} aria-hidden="true" />{t('home.createInstance')}</button>
      </EmptyState>
    </div>
  {/if}
</div>

<style>
  /* transparent: the shell's background engine shows through the hero; the page below the hero is opaque */
  .home { height: 100%; overflow: auto; display: flex; flex-direction: column; }
  /* hero: full bleed to the content edge, ~46% of the viewport */
  .hero { position: relative; flex: none; min-height: 46vh; display: flex; align-items: flex-end; }
  .content { position: relative; z-index: 1; display: grid; gap: 14px; justify-items: start; padding: 40px 40px 44px 44px; width: 100%; }
  .status { display: inline-flex; align-items: center; gap: 8px; color: var(--paper-dim); }
  .idrow { display: flex; align-items: flex-end; gap: 20px; min-width: 0; }
  .idtext { display: grid; gap: 14px; justify-items: start; min-width: 0; }
  .iconbtn { position: relative; flex: none; display: block; width: 96px; height: 96px; margin-bottom: 4px; }
  .iconbtn:focus-visible { outline: 2px solid var(--focus); outline-offset: 4px; }
  .pen { position: absolute; right: 4px; bottom: 4px; width: 24px; height: 24px; display: grid; place-items: center; background: var(--accent-fill); color: var(--on-fill); opacity: 0; transform: translateY(3px); transition: opacity var(--dur) var(--ease), transform var(--dur) var(--ease); }
  .iconbtn:hover .pen, .iconbtn:focus-visible .pen { opacity: 1; transform: none; }
  .bgbtn { position: absolute; right: 16px; top: 14px; width: 36px; padding: 0; background: rgba(var(--scrim), 0.55); border-color: var(--hero-line); color: var(--paper); z-index: 2; }
  .live-t { color: var(--leaf); }
  .live { animation: pulse 1.6s ease-in-out infinite; }
  h1 { font-size: var(--fs-hero); max-width: 14ch; text-wrap: balance; color: var(--paper); }
  .info { margin: 0; color: var(--paper-dim); font-size: var(--fs-md); }
  .actions { display: flex; flex-wrap: wrap; gap: 8px 16px; align-items: center; margin-top: 6px; }
  .total { position: absolute; right: 40px; bottom: 44px; display: grid; justify-items: end; gap: 4px; }
  .total .n { font-size: var(--fs-2xl); color: var(--paper); letter-spacing: -0.02em; font-feature-settings: 'tnum'; }
  .pad { padding: var(--s-4) 32px 0; }

  .body { flex: 1; background: var(--ink-0); display: grid; grid-template-columns: minmax(0, 1fr); gap: var(--s-6); padding: var(--s-5) 32px 32px 44px; }
  header { display: flex; justify-content: space-between; align-items: baseline; margin-bottom: var(--s-3); }
  h2 { font-size: var(--fs-xl); }
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
