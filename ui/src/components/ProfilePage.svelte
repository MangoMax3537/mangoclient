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
  import { enter } from '../lib/motion';
  import { rememberScroll } from '../lib/scrollMemory';
  import NoAccountHint from './NoAccountHint.svelte';
  import { fmtDuration, fmtRam } from '../lib/format';
  import type { Key } from '../lib/i18n';
  import { untrack } from 'svelte';
  import { createLoadState } from '../lib/loadState.svelte';
  import { CONTENT_TAB_TYPE, effectiveTab, isContentTab, profileTabs } from '../lib/profileTabs';
  import { actions, app, contentRev, getApi, isBusy, nav, openBackgroundPicker, openIconPicker, t, type ProfileTab } from '../lib/stores.svelte';
  import type { ContentItem, ContentType, UpdateInfo } from '../lib/types';

  let { id }: { id: string } = $props();
  const p = $derived(app.profiles.find((x) => x.id === id) ?? null);
  const busy = $derived(isBusy(id));
  const ram = $derived(p ? p.effectiveRamMb : 0);
  const loaderLabel = $derived(p ? (p.loader === 'vanilla' ? t('profile.vanilla') : `${p.loader} ${p.loaderVersion ?? ''}`.trim()) : '');

  // The instance's content (all types) and its update infos feed the tab counts and the four content tabs.
  const cl = createLoadState<ContentItem[]>({ isEmpty: () => false });
  let items = $state<ContentItem[] | null>(null);
  let infos = $state<UpdateInfo[]>([]);
  $effect(() => {
    const pid = id;
    void contentRev[pid];
    cl.run(async () => {
      const l = await getApi().contentList({ profileId: pid });
      getApi().contentCheckUpdates({ profileId: pid }).then((u) => { if (pid === id) infos = u; }, () => {});
      return l;
    }, { silent: untrack(() => items !== null) });
    return () => cl.cancel();
  });
  $effect(() => { if (cl.data) items = cl.data; });
  const counts = $derived.by(() => {
    const c: Record<ContentType, number> = { mod: 0, shader: 0, resourcepack: 0, datapack: 0 };
    for (const i of items ?? []) c[i.type]++;
    return c;
  });
  const tabList = $derived(p ? profileTabs(p.loader, counts) : []);
  const tab = $derived(effectiveTab(nav.profileTab, tabList, items !== null));
  const opts = $derived(tabList.map((k) => {
    const ty = isContentTab(k) ? CONTENT_TAB_TYPE[k] : null;
    return { value: k, label: t(`profile.tab.${k}` as Key), ...(ty && items && counts[ty] > 0 ? { count: counts[ty] } : {}) };
  }));

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
    <section class="hero on-stage">
      <PanoramaBackground profileId={p.id} mcVersion={p.mcVersion} background={p.background} tint={p.color} />
      <div class="inner">
        <div class="top">
          <button class="btn ghost back" type="button" onclick={() => (nav.view = 'library')}><ArrowLeftIcon weight="bold" size={14} aria-hidden="true" />{t('nav.library')}</button>
          <div class="tools">
            <button class="bgbtn btn" type="button" use:tip={{ text: t('bg.change'), side: 'top' }} aria-label={t('bg.change')} onclick={() => openBackgroundPicker(p.id)}><ImageIcon weight="bold" size={16} aria-hidden="true" /></button>
            <ProfileMoreMenu profileId={p.id} name={p.name} />
          </div>
        </div>
        <div class="row">
          <button class="iconbtn" type="button" use:tip={{ text: t('icons.change'), side: 'top' }} aria-label={t('icons.change')} onclick={() => openIconPicker(p.id)}>
            <InstanceIcon profile={p} size={84} notch={13} />
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

    <div class="tabs"><Seg options={opts} value={tab} label={p.name} role="tablist" idPrefix="ptab" onchange={(v) => (nav.profileTab = v as ProfileTab)} /></div>

    <div class="panel" role="tabpanel" aria-labelledby="ptab-{tab}" use:rememberScroll={`${p.id}:${tab}`}>
      {#key `${p.id}:${tab}`}
        <div class="tabbody" in:enter={{ y: 6, duration: 160 }}>
        {#if tab === 'overview'}<Overview profile={p} />
        {:else if isContentTab(tab)}<Lazy load={tabs.content} props={{ profile: p, type: CONTENT_TAB_TYPE[tab], items, infos, load: cl, onitems: (l: ContentItem[]) => (items = l) }} fallback={skContent} />
        {:else if tab === 'media'}<Lazy load={tabs.media} props={{ profile: p }} fallback={skMedia} />
        {:else if tab === 'backups'}<Lazy load={tabs.backups} props={{ profile: p }} fallback={skBackups} />
        {:else if tab === 'logs'}<Lazy load={tabs.logs} props={{ profile: p }} fallback={skLogs} />
        {:else}<Lazy load={tabs.settings} props={{ profile: p }} fallback={skSettings} />{/if}
        </div>
      {/key}
    </div>
  </div>
{/if}

<style>
  .page { display: flex; flex-direction: column; height: 100%; min-height: 0; padding: 16px 20px 0; gap: 12px; }
  /* The hero is the same on every tab: back/tools row on top, identity and Play below. Nothing is positioned absolutely, so nothing can overlap,
     and the Play column wraps under the identity when the window is narrow. */
  .hero { position: relative; flex: none; border-radius: var(--r-l); overflow: hidden; border: 1px solid var(--line); display: flex; }
  .inner { position: relative; width: 100%; min-width: 0; display: flex; flex-direction: column; gap: 14px; padding: 10px 20px 18px 28px; }
  .top { display: flex; align-items: center; justify-content: space-between; gap: 12px; }
  .back { height: 28px; margin-left: -10px; }
  .tools { display: flex; gap: 6px; }
  .bgbtn { width: 34px; height: 30px; padding: 0; background: rgba(var(--scrim), 0.55); border-color: var(--hero-line); color: #f4f1ea; }
  .row { display: flex; align-items: flex-end; gap: 16px 20px; flex-wrap: wrap; min-width: 0; }
  .id { display: grid; gap: 10px; min-width: 0; flex: 1 1 320px; }
  .iconbtn { position: relative; flex: none; display: block; }
  .iconbtn:focus-visible { outline: 2px solid var(--focus); outline-offset: 4px; border-radius: var(--r-s); }
  .pen { position: absolute; right: 3px; bottom: 3px; width: 20px; height: 20px; display: grid; place-items: center; background: var(--accent-fill); color: var(--on-fill); border-radius: var(--r-s); opacity: 0; transform: translateY(3px); transition: opacity var(--dur) var(--ease), transform var(--dur) var(--ease); }
  .iconbtn:hover .pen, .iconbtn:focus-visible .pen { opacity: 1; transform: none; }
  h1 { font-size: clamp(28px, 4vw, 42px); line-height: 1.05; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; max-width: 100%; }
  .chips { display: flex; flex-wrap: wrap; gap: 8px; list-style: none; margin: 0; padding: 0; }
  .playcol { display: grid; gap: var(--s-2); justify-items: end; flex: 0 1 auto; max-width: min(100%, 300px); }
  /* short windows (960x600): the same hero, a little tighter, so the tab content keeps its room */
  @media (max-height: 700px) { .inner { gap: 6px; padding: 8px 20px 12px 28px; } h1 { font-size: clamp(26px, 3.4vw, 34px); } .chips { gap: 6px; } }
  .tabs { flex: none; max-width: 100%; }
  .panel { flex: 1; min-height: 0; overflow: auto; padding: 4px 0 20px; }
</style>
