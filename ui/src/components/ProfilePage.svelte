<script lang="ts">
  import ArrowLeftIcon from 'phosphor-svelte/lib/ArrowLeftIcon';
  import CubeIcon from 'phosphor-svelte/lib/CubeIcon';
  import CoffeeIcon from 'phosphor-svelte/lib/CoffeeIcon';
  import MemoryIcon from 'phosphor-svelte/lib/MemoryIcon';
  import ClockIcon from 'phosphor-svelte/lib/ClockIcon';
  import PlayButton from './PlayButton.svelte';
  import Seg from './Seg.svelte';
  import Overview from './profile/Overview.svelte';
  import Lazy from './Lazy.svelte';
  import { skeletons, tabs } from '../lib/lazy';
  import PanoramaBackground from './PanoramaBackground.svelte';
  import InstanceIcon from './InstanceIcon.svelte';
  import ProfileMoreMenu from './ProfileMoreMenu.svelte';
  import PencilSimpleIcon from 'phosphor-svelte/lib/PencilSimpleIcon';
  import ImageIcon from 'phosphor-svelte/lib/ImageIcon';
  import { tip } from '../lib/tooltip';
  import NoAccountHint from './NoAccountHint.svelte';
  import { fmtDuration, fmtRam } from '../lib/format';
  import type { Key } from '../lib/i18n';
  import { actions, app, isBusy, nav, openBackgroundPicker, openIconPicker, t, type ProfileTab } from '../lib/stores.svelte';

  let { id }: { id: string } = $props();
  const p = $derived(app.profiles.find((x) => x.id === id) ?? null);
  const busy = $derived(isBusy(id));
  const compact = $derived(nav.profileTab !== 'overview');
  const TABS: ProfileTab[] = ['overview', 'content', 'media', 'backups', 'logs', 'settings'];
  const opts = $derived(TABS.map((k) => ({ value: k, label: t(`profile.tab.${k}` as Key) })));
  const ram = $derived(p ? p.effectiveRamMb : 0);
  const loaderLabel = $derived(p ? (p.loader === 'vanilla' ? t('profile.vanilla') : `${p.loader} ${p.loaderVersion ?? ''}`.trim()) : '');

  $effect(() => { if (!p && app.ready) { nav.view = 'library'; } });
  function toggle() {
    if (!p) return;
    if (busy) actions.stop(p.id);
    else { actions.selectProfile(p.id); actions.play(p.id); }
  }
</script>

{#snippet skContent()}<Lazy load={skeletons.profileTab} props={{ tab: 'content' }} />{/snippet}
{#snippet skMedia()}<Lazy load={skeletons.profileTab} props={{ tab: 'media' }} />{/snippet}
{#snippet skBackups()}<Lazy load={skeletons.profileTab} props={{ tab: 'backups' }} />{/snippet}
{#snippet skLogs()}<Lazy load={skeletons.profileTab} props={{ tab: 'logs' }} />{/snippet}
{#snippet skSettings()}<Lazy load={skeletons.profileTab} props={{ tab: 'settings' }} />{/snippet}

{#if p}
  <div class="page">
    <section class="hero on-stage" class:compact={nav.profileTab !== 'overview'}>
      <PanoramaBackground profileId={p.id} mcVersion={p.mcVersion} background={p.background} tint={p.color} />
      <div class="tools">
        <button class="bgbtn btn" type="button" use:tip={{ text: t('bg.change'), side: 'top' }} aria-label={t('bg.change')} onclick={() => openBackgroundPicker(p.id)}><ImageIcon weight="bold" size={16} aria-hidden="true" /></button>
        <ProfileMoreMenu profileId={p.id} name={p.name} />
      </div>
      <div class="inner">
        <button class="btn ghost back" type="button" onclick={() => (nav.view = 'library')}><ArrowLeftIcon weight="bold" size={14} aria-hidden="true" />{t('nav.library')}</button>
        <div class="row">
          <button class="iconbtn" type="button" use:tip={{ text: t('icons.change'), side: 'top' }} aria-label={t('icons.change')} onclick={() => openIconPicker(p.id)}>
            <InstanceIcon profile={p} size={compact ? 44 : 84} notch={compact ? 8 : 13} />
            <span class="pen" aria-hidden="true"><PencilSimpleIcon weight="bold" size={12} /></span>
          </button>
          <div class="id">
            <h1>{p.name}</h1>
            <ul class="chips" aria-label={p.name}>
              <li class="chip mono"><CubeIcon weight="bold" size={14} aria-hidden="true" />{p.mcVersion}</li>
              <li class="chip">{loaderLabel}</li>
              <li class="chip mono"><CoffeeIcon weight="bold" size={14} aria-hidden="true" />{t('chip.java')} {t('chip.auto')}</li>
              <li class="chip mono"><MemoryIcon weight="bold" size={14} aria-hidden="true" />{ram ? fmtRam(ram) : t('chip.default')}</li>
              <li class="chip mono"><ClockIcon weight="bold" size={14} aria-hidden="true" />{fmtDuration(p.playTimeMs)}</li>
            </ul>
          </div>
          <div class="playcol">
            <PlayButton mode={busy ? 'stop' : 'play'} label={busy ? t('home.stop') : t('home.play')} onclick={toggle} disabled={!app.accounts.length && !busy} />
            {#if !app.accounts.length && !busy}<NoAccountHint />{/if}
          </div>
        </div>
      </div>
    </section>

    <div class="tabs"><Seg options={opts} value={nav.profileTab} label={p.name} role="tablist" idPrefix="ptab" onchange={(v) => (nav.profileTab = v as ProfileTab)} /></div>

    <div class="panel" role="tabpanel" aria-labelledby="ptab-{nav.profileTab}">
      {#key p.id}
        {#if nav.profileTab === 'overview'}<Overview profile={p} />
        {:else if nav.profileTab === 'content'}<Lazy load={tabs.content} props={{ profile: p }} fallback={skContent} />
        {:else if nav.profileTab === 'media'}<Lazy load={tabs.media} props={{ profile: p }} fallback={skMedia} />
        {:else if nav.profileTab === 'backups'}<Lazy load={tabs.backups} props={{ profile: p }} fallback={skBackups} />
        {:else if nav.profileTab === 'logs'}<Lazy load={tabs.logs} props={{ profile: p }} fallback={skLogs} />
        {:else}<Lazy load={tabs.settings} props={{ profile: p }} fallback={skSettings} />{/if}
      {/key}
    </div>
  </div>
{/if}

<style>
  .page { display: flex; flex-direction: column; height: 100%; min-height: 0; padding: 16px 20px 0; gap: 12px; }
  .hero { position: relative; flex: none; min-height: 190px; border-radius: var(--r-l); overflow: hidden; border: 1px solid var(--line); display: flex; }
  .inner { position: relative; width: 100%; display: flex; flex-direction: column; justify-content: space-between; padding: 10px 24px 18px 28px; gap: 12px; }
  .back { align-self: flex-start; height: 28px; margin-left: -10px; }
  .row { display: flex; align-items: flex-end; gap: 16px; flex-wrap: wrap; }
  .id { display: grid; gap: 10px; min-width: 0; flex: 1; }
  .iconbtn { position: relative; flex: none; display: block; }
  .iconbtn:focus-visible { outline: 2px solid var(--focus); outline-offset: 4px; border-radius: var(--r-s); }
  .pen { position: absolute; right: 3px; bottom: 3px; width: 20px; height: 20px; display: grid; place-items: center; background: var(--accent-fill); color: var(--on-fill); border-radius: var(--r-s); opacity: 0; transform: translateY(3px); transition: opacity var(--dur) var(--ease), transform var(--dur) var(--ease); }
  .iconbtn:hover .pen, .iconbtn:focus-visible .pen { opacity: 1; transform: none; }
  .tools { position: absolute; right: 12px; top: 10px; z-index: 3; display: flex; gap: 6px; }
  .bgbtn { width: 34px; height: 30px; padding: 0; background: rgba(var(--scrim), 0.55); border-color: var(--hero-line); color: #f4f1ea; z-index: 2; }
  h1 { font-size: clamp(28px, 4vw, 42px); line-height: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; max-width: 100%; }
  .chips { display: flex; flex-wrap: wrap; gap: 8px; list-style: none; margin: 0; padding: 0; }
  .hero.compact { min-height: 0; }
  .hero.compact .back, .hero.compact .chips { display: none; }
  .hero.compact .inner { padding: 14px 20px 14px 28px; }
  .hero.compact h1 { font-size: 26px; }
  .playcol { display: grid; gap: var(--s-2); justify-items: end; }
  .tabs { flex: none; }
  .panel { flex: 1; min-height: 0; overflow: auto; padding: 4px 0 20px; }
</style>
