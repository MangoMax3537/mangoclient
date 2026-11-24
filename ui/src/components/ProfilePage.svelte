<script lang="ts">
  import ArrowLeftIcon from 'phosphor-svelte/lib/ArrowLeftIcon';
  import PlayButton from './PlayButton.svelte';
  import Seg from './Seg.svelte';
  import SquaresFourIcon from 'phosphor-svelte/lib/SquaresFourIcon';
  import PuzzlePieceIcon from 'phosphor-svelte/lib/PuzzlePieceIcon';
  import SunHorizonIcon from 'phosphor-svelte/lib/SunHorizonIcon';
  import PaletteIcon from 'phosphor-svelte/lib/PaletteIcon';
  import FileCodeIcon from 'phosphor-svelte/lib/FileCodeIcon';
  import ImagesIcon from 'phosphor-svelte/lib/ImagesIcon';
  import ArchiveIcon from 'phosphor-svelte/lib/ArchiveIcon';
  import ScrollIcon from 'phosphor-svelte/lib/ScrollIcon';
  import GearIcon from 'phosphor-svelte/lib/GearIcon';
  import Overview from './profile/Overview.svelte';
  import Lazy from './Lazy.svelte';
  import { skeletons, tabs } from '../lib/lazy';
  import PanoramaBackground from './PanoramaBackground.svelte';
  import InstanceIcon from './InstanceIcon.svelte';
  import ProfileMoreMenu from './ProfileMoreMenu.svelte';
  import PencilSimpleIcon from 'phosphor-svelte/lib/PencilSimpleIcon';
  import ImageIcon from 'phosphor-svelte/lib/ImageIcon';
  import { tip } from '../lib/tooltip';
  import { contextmenu } from '../lib/contextMenu.svelte';
  import { pageMenu } from '../lib/instanceMenu';
  import { checkName, problemKey } from '../lib/rename';
  import { enter } from '../lib/motion';
  import { rememberScroll } from '../lib/scrollMemory';
  import NoAccountHint from './NoAccountHint.svelte';
  import { heroFacts } from '../lib/infoLine';
  import InfoLine from './InfoLine.svelte';
  import type { Key } from '../lib/i18n';
  import { untrack } from 'svelte';
  import { createLoadState } from '../lib/loadState.svelte';
  import { CONTENT_TAB_TYPE, effectiveTab, isContentTab, profileTabs } from '../lib/profileTabs';
  import { actions, app, contentRev, errorText, getApi, isBusy, nav, openBackgroundPicker, openIconPicker, lang, t, toast, ui, type ProfileTab, stopping } from '../lib/stores.svelte';
  import type { ContentItem, ContentType, UpdateInfo } from '../lib/types';

  let { id }: { id: string } = $props();
  const p = $derived(app.profiles.find((x) => x.id === id) ?? null);
  const busy = $derived(isBusy(id));

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
  const TAB_ICON: Record<ProfileTab, typeof GearIcon> = {
    overview: SquaresFourIcon, mods: PuzzlePieceIcon, shaders: SunHorizonIcon, resourcepacks: PaletteIcon, datapacks: FileCodeIcon,
    media: ImagesIcon, backups: ArchiveIcon, logs: ScrollIcon, settings: GearIcon,
  };
  const opts = $derived(tabList.map((k) => {
    const ty = isContentTab(k) ? CONTENT_TAB_TYPE[k] : null;
    return { value: k, label: t(`profile.tab.${k}` as Key), icon: TAB_ICON[k], ...(ty && items && counts[ty] > 0 ? { count: counts[ty] } : {}) };
  }));

  /* inline rename of the title: double click, F2 or the menu; Enter saves, Esc cancels, the problem shows under the field */
  let draft = $state('');
  let nameInput: HTMLInputElement | undefined = $state();
  let renameErr = $state('');
  let saving = false;
  const editing = $derived(!!p && ui.renameInline === p.id);
  $effect(() => {
    if (editing && p) { draft = p.name; renameErr = ''; queueMicrotask(() => { nameInput?.focus(); nameInput?.select(); }); }
  });
  const stopRename = () => { ui.renameInline = null; renameErr = ''; };
  async function commitRename() {
    if (!p || saving || ui.renameInline !== p.id) return;
    const c = checkName(draft);
    if (!c.ok) { renameErr = t(problemKey(c.problem)); return; }
    if (c.name === p.name) { stopRename(); return; }
    saving = true;
    try { await actions.updateProfile(p.id, { name: c.name }); toast(t('rename.done', { name: c.name }), 'ok', 2500); stopRename(); }
    catch (e) { renameErr = errorText(e); }
    saving = false;
  }
  function renameKey(e: KeyboardEvent) {
    if (e.isComposing) return;
    if (e.key === 'Enter') { e.preventDefault(); void commitRename(); }
    else if (e.key === 'Escape') { e.preventDefault(); e.stopPropagation(); stopRename(); }
  }

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
    <section class="hero on-stage notch" style:--n="18px" use:contextmenu={() => pageMenu(p)}>
      <!-- `auto` shows the one global engine behind the whole window (it keeps cycling across views and tabs); only an explicit choice gets its own layer -->
      {#if p.background.kind !== 'auto'}<PanoramaBackground profileId={p.id} mcVersion={p.mcVersion} background={p.background} tint={p.color} />{/if}
      <div class="inner">
        <div class="top">
          <button class="btn ghost back" type="button" onclick={() => (nav.view = 'library')}><ArrowLeftIcon weight="bold" size={14} aria-hidden="true" />{t('nav.library')}</button>
          <div class="tools">
            <button class="bgbtn btn" type="button" use:tip={{ text: t('bg.change'), side: 'top' }} aria-label={t('bg.change')} onclick={() => openBackgroundPicker(p.id)}><ImageIcon weight="bold" size={16} aria-hidden="true" /></button>
            <ProfileMoreMenu profile={p} />
          </div>
        </div>
        <div class="row">
          <button class="iconbtn" type="button" use:tip={{ text: t('icons.change'), side: 'top' }} aria-label={t('icons.change')} onclick={() => openIconPicker(p.id)}>
            <InstanceIcon profile={p} size={84} notch={13} />
            <span class="pen" aria-hidden="true"><PencilSimpleIcon weight="bold" size={12} /></span>
          </button>
          <div class="id">
            {#if editing}
              <div class="rn">
                <input bind:this={nameInput} class="field rn-in" type="text" bind:value={draft} maxlength="256" spellcheck="false" autocomplete="off" aria-label={t('rename.inline', { name: p.name })}
                  aria-invalid={renameErr ? 'true' : undefined} onkeydown={renameKey} oninput={() => (renameErr = '')} onblur={() => void commitRename().then(() => { if (renameErr) stopRename(); })} />
                {#if renameErr}<div class="err" role="alert">{renameErr}</div>{/if}
              </div>
            {:else}
              <h1 title={t('rename.hint')} ondblclick={() => (ui.renameInline = p.id)}>{p.name}</h1>
            {/if}
            <p class="info mono"><InfoLine facts={heroFacts(p, lang(), t)} size={15} /></p>
          </div>
          <div class="playcol">
            <PlayButton mode={busy ? 'stop' : 'play'} label={busy ? (stopping[p.id] ? t('home.stopping') : t('home.stop')) : t('home.play')} onclick={toggle} disabled={!app.accounts.length && !busy} />
            {#if !app.accounts.length && !busy}<NoAccountHint />{/if}
          </div>
        </div>
      </div>
    </section>

    <div class="tabs"><Seg options={opts} value={tab} label={p.name} role="tablist" idPrefix="ptab" onchange={(v) => (nav.profileTab = v as ProfileTab)} /></div>

    <div class="panel" role="tabpanel" aria-labelledby="ptab-{tab}" use:rememberScroll={`${p.id}:${tab}`}>
      {#key `${p.id}:${tab}`}
        <div class="tabbody" in:enter={{ y: 6, duration: 160 }}>
        {#if tab === 'overview'}<Overview profile={p} modCount={items ? counts.mod : p.modCount} />
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
  .hero { background: linear-gradient(rgba(11, 12, 14, 0.5), rgba(11, 12, 14, 0.5)); position: relative; flex: none; overflow: hidden; display: flex; }
  .inner { position: relative; width: 100%; min-width: 0; display: flex; flex-direction: column; gap: 14px; padding: 10px 20px 18px 28px; }
  .top { display: flex; align-items: center; justify-content: space-between; gap: 12px; }
  .back { height: 28px; margin-left: -10px; }
  .tools { display: flex; gap: 6px; }
  .bgbtn { width: 34px; height: 30px; padding: 0; background: rgba(var(--scrim), 0.55); border-color: var(--hero-line); color: #f4f1ea; }
  .row { display: flex; align-items: flex-end; gap: 16px 20px; flex-wrap: wrap; min-width: 0; }
  .id { display: grid; gap: 10px; min-width: 0; flex: 1 1 320px; }
  .iconbtn { position: relative; flex: none; display: block; }
  .iconbtn:focus-visible { outline: 2px solid var(--focus); outline-offset: 4px; }
  .pen { position: absolute; right: 3px; bottom: 3px; width: 20px; height: 20px; display: grid; place-items: center; background: var(--accent-fill); color: var(--on-fill); opacity: 0; transform: translateY(3px); transition: opacity var(--dur) var(--ease), transform var(--dur) var(--ease); }
  .iconbtn:hover .pen, .iconbtn:focus-visible .pen { opacity: 1; transform: none; }
  h1 { font-size: clamp(28px, 4vw, 42px); line-height: 1.05; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; max-width: 100%; }
  .rn { display: grid; gap: 4px; min-width: 0; }
  .rn-in { height: 48px; font: 700 clamp(24px, 3.4vw, 36px)/1.05 var(--font-display); letter-spacing: -0.015em; color: var(--paper); background: rgba(var(--scrim), 0.6); border-color: var(--hero-line); }
  .info { margin: 0; color: var(--hero-dim); font-size: var(--fs-md); }
  .playcol { display: grid; gap: var(--s-2); justify-items: end; flex: 0 1 auto; max-width: min(100%, 300px); }
  /* short windows (960x600): the same hero, a little tighter, so the tab content keeps its room */
  @media (max-height: 700px) { .inner { gap: 6px; padding: 8px 20px 12px 28px; } h1 { font-size: clamp(26px, 3.4vw, 34px); } }
  .tabs { flex: none; max-width: 100%; }
  /* a flex column so a tab can fill the panel with `flex: 1` (WebKitGTK does not resolve `height: 100%` against a flexed parent) */
  .panel { flex: 1; min-height: 0; overflow: auto; padding: 4px 0 20px; display: flex; flex-direction: column; }
</style>
