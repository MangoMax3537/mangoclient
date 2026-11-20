<script lang="ts">
  import MagnifyingGlassIcon from 'phosphor-svelte/lib/MagnifyingGlassIcon';
  import FolderOpenIcon from 'phosphor-svelte/lib/FolderOpenIcon';
  import TrashIcon from 'phosphor-svelte/lib/TrashIcon';
  import ArrowCircleUpIcon from 'phosphor-svelte/lib/ArrowCircleUpIcon';
  import LinkIcon from 'phosphor-svelte/lib/LinkIcon';
  import Tile from '../Tile.svelte';
  import Seg from '../Seg.svelte';
  import VirtualList from '../VirtualList.svelte';
  import ConfirmDialog from '../ConfirmDialog.svelte';
  import EmptyState from '../EmptyState.svelte';
  import PlusIcon from 'phosphor-svelte/lib/PlusIcon';
  import type { Key } from '../../lib/i18n';
  import { providerLabel } from '../../lib/install';
  import { contentRev, errorText, getApi, installs, openDiscover, startInstall, t, toast, tp } from '../../lib/stores.svelte';
  import type { ContentItem, ContentType, ProfileView, UpdateInfo } from '../../lib/types';

  let { profile }: { profile: ProfileView } = $props();
  type Filter = 'all' | ContentType;

  let items = $state<ContentItem[] | null>(null);
  let updateInfos = $state<UpdateInfo[]>([]);
  let query = $state('');
  let filter = $state<Filter>('all');
  let pending = $state<Record<string, boolean>>({});
  let confirmItem = $state<ContentItem | null>(null);
  let blocked = $state<{ item: ContentItem; dependents: string[] } | null>(null);
  let confirmOff = $state<ContentItem | null>(null);
  let working = $state(false);
  let loadError = $state('');

  const title = (id: string) => items?.find((i) => i.projectId === id)?.title ?? id;
  const counts = $derived.by(() => {
    const c: Record<Filter, number> = { all: items?.length ?? 0, mod: 0, shader: 0, resourcepack: 0, datapack: 0 };
    for (const i of items ?? []) c[i.type]++;
    return c;
  });
  const opts = $derived(
    (['all', 'mod', 'shader', 'resourcepack', 'datapack'] as Filter[])
      .filter((f) => f === 'all' || counts[f] > 0 || f === filter)
      .map((f) => ({ value: f, label: t(`content.type.${f}` as Key), count: counts[f] })),
  );
  const shown = $derived.by(() => {
    const q = query.trim().toLowerCase();
    return (items ?? [])
      .filter((i) => (filter === 'all' || i.type === filter) && (!q || i.title.toLowerCase().includes(q) || i.filename.toLowerCase().includes(q)))
      .sort((a, b) => a.title.localeCompare(b.title));
  });
  const updatable = $derived((items ?? []).filter((i) => i.updateAvailable));

  async function load() {
    const id = profile.id;
    loadError = '';
    try {
      const l = await getApi().contentList({ profileId: id });
      if (id !== profile.id) return;
      items = l;
      getApi().contentCheckUpdates({ profileId: id }).then((u) => { if (id === profile.id) updateInfos = u; }, () => {});
    } catch (e) { loadError = errorText(e); items = []; }
  }
  $effect(() => { void profile.id; void contentRev[profile.id]; load(); });

  async function toggle(it: ContentItem, enabled: boolean) {
    pending[it.projectId] = true;
    try {
      const r = await getApi().contentToggle({ profileId: profile.id, projectId: it.projectId, enabled });
      items = r.items;
      if (r.affected.length) toast(t(enabled ? 'content.alsoEnabled' : 'content.alsoDisabled', { list: r.affected.map(title).join(', ') }), 'info');
    } catch (e) { toast(errorText(e), 'error'); }
    delete pending[it.projectId];
  }
  async function uninstall(it: ContentItem) {
    working = true;
    try {
      const r = await getApi().contentUninstall({ profileId: profile.id, projectId: it.projectId });
      confirmItem = null;
      if (r.blocked) blocked = { item: it, dependents: r.dependents };
      else { items = r.items; toast(t('content.uninstalled', { name: it.title }), 'ok'); }
    } catch (e) { toast(errorText(e), 'error'); confirmItem = null; }
    working = false;
  }
  function askUninstall(it: ContentItem) {
    if (it.requiredBy.length) uninstall(it); // the backend answers with the blocking dependents
    else confirmItem = it;
  }
  /** Single update: straight to the dependency dialog for this instance (picker skipped), showing what the new version needs. */
  function update(it: ContentItem) {
    const info = updateInfos.find((u) => u.projectId === it.projectId);
    if (!it.provider) return;
    startInstall({
      kind: 'content', update: true, scopeProfileId: profile.id, versionId: info?.versionId,
      project: { provider: it.provider, projectId: it.projectId, title: it.title, type: it.type, iconUrl: it.iconUrl },
    });
  }
  /** Update all: first a preview of versions and new dependencies, nothing is applied before the confirmation. */
  function updateAll() { startInstall({ kind: 'updates', profileId: profile.id, infos: updateInfos.filter((u) => u.provider) }); }
  /** Disabling something other content needs cascades; say so before it happens. */
  function askToggle(it: ContentItem, enabled: boolean) {
    const live = it.requiredBy.filter((id) => items?.find((i) => i.projectId === id)?.enabled);
    if (!enabled && live.length) confirmOff = it; else toggle(it, enabled);
  }
  async function openFolder() {
    try { toast(await getApi().contentOpenFolder({ profileId: profile.id, type: filter === 'all' ? 'mod' : filter }), 'info'); } catch (e) { toast(errorText(e), 'error'); }
  }
  const browseType = $derived(filter === 'shader' ? 'shader' : filter === 'resourcepack' ? 'resourcepack' : 'mod');
</script>

<div class="ct">
  <div class="bar">
    <Seg options={opts} value={filter} label={t('content.filter')} idPrefix="cf" onchange={(v) => (filter = v as Filter)} />
    <label class="search">
      <MagnifyingGlassIcon weight="bold" size={16} aria-hidden="true" />
      <span class="sr-only">{t('content.search')}</span>
      <input class="field" type="search" placeholder={t('content.search')} bind:value={query} />
    </label>
    <button class="btn" type="button" onclick={openFolder}><FolderOpenIcon weight="bold" size={16} aria-hidden="true" />{t('content.openFolder')}</button>
    <button class="btn accent" type="button" onclick={() => openDiscover(profile.id, browseType)}><PlusIcon weight="bold" size={16} aria-hidden="true" />{t('content.add')}</button>
  </div>

  {#if updatable.length}
    <div class="note" role="status">
      <ArrowCircleUpIcon weight="bold" size={16} aria-hidden="true" />
      <span>{tp('content.updatesAvailable', updatable.length)}</span>
      <button class="btn" type="button" disabled={!updateInfos.length} onclick={updateAll}>{t('content.updateAll')}</button>
    </div>
  {/if}

  {#if items === null}
    <ul class="skel" aria-label={t('common.loading')}>{#each Array(8) as _, i (i)}<li><span class="img"></span><span class="bar1"></span></li>{/each}</ul>
  {:else if loadError}
    <p class="err">{loadError}</p>
  {:else if items.length === 0}
    <EmptyState title={t('content.emptyTitle')} hint={t('content.empty')}>
      <button class="btn accent" type="button" onclick={() => openDiscover(profile.id, browseType)}><PlusIcon weight="bold" size={16} aria-hidden="true" />{t('content.add')}</button>
    </EmptyState>
  {:else if shown.length === 0}
    <EmptyState title={t('content.noMatch')} hint={t('library.emptyHint')}>
      <button class="btn" type="button" onclick={() => { query = ''; filter = 'all'; }}>{t('library.clearSearch')}</button>
    </EmptyState>
  {:else}
    <div class="list">
      <VirtualList items={shown} rowHeight={62} key={(i) => i.projectId} label={t('profile.tab.content')}>
        {#snippet row(it)}
          {@const inst = installs[it.projectId]}
          <div class="item" class:off={!it.enabled}>
            <Tile kind="mod" id={it.projectId} src={it.iconUrl} size={40} radius={6} />
            <div class="main">
              <div class="l1">
                <b class="nm">{it.title}</b>
                <span class="ver mono">{it.versionNumber ?? '?'}</span>
                <span class="tag">{t(`content.type.${it.type}` as Key)}</span>
                {#if it.dependency}<span class="tag dep">{t('content.dependency')}</span>{/if}
                {#if it.provider}<span class="tag prov">{providerLabel(it.provider)}</span>{:else}<span class="tag">{t('content.local')}</span>{/if}
              </div>
              <div class="l2">
                {#if it.requiredBy.length}
                  <LinkIcon weight="bold" size={12} aria-hidden="true" />
                  <span class="sr-only">{t('content.requiredBy')}</span>
                  <span class="rb" title={it.requiredBy.map(title).join(', ')}>{t('content.requiredBy')}</span>
                  {#each it.requiredBy.slice(0, 3) as r (r)}<span class="chip sm">{title(r)}</span>{/each}
                  {#if it.requiredBy.length > 3}<span class="chip sm">+{it.requiredBy.length - 3}</span>{/if}
                {:else}
                  <span class="fn mono" title={it.filename}>{it.filename}</span>
                {/if}
              </div>
            </div>
            {#if it.updateAvailable}
              <button class="btn upd" type="button" disabled={!!inst} onclick={() => update(it)} aria-label={t('content.updateTo', { name: it.title, v: it.updateAvailable })}>
                <ArrowCircleUpIcon weight="bold" size={14} aria-hidden="true" />{inst ? `${Math.round((inst.done / inst.total) * 100)}%` : it.updateAvailable}
              </button>
            {:else}<span></span>{/if}
            <span class="switch"><input type="checkbox" checked={it.enabled} disabled={pending[it.projectId]} aria-label="{t('content.enabled')}: {it.title}" onchange={(e) => { const on = e.currentTarget.checked; if (!on && it.requiredBy.length) { e.currentTarget.checked = true; } askToggle(it, on); }} /><span></span></span>
            <button class="btn ghost ic" type="button" aria-label="{t('content.uninstall')}: {it.title}" title={t('content.uninstall')} onclick={() => askUninstall(it)}><TrashIcon weight="bold" size={16} aria-hidden="true" /></button>
          </div>
        {/snippet}
      </VirtualList>
    </div>
  {/if}
</div>

{#if confirmItem}
  <ConfirmDialog title={t('content.uninstallTitle', { name: confirmItem.title })} body={t('content.uninstallBody', { file: confirmItem.filename })} facts={[{ label: t('col.version'), value: confirmItem.versionNumber ?? '?' }, { label: t('content.source'), value: confirmItem.provider ? providerLabel(confirmItem.provider) : t('content.local') }]} warn={confirmItem.dependency ? t('content.uninstallDepWarn') : ''} confirmLabel={t('content.uninstall')} danger busy={working}
    onconfirm={() => confirmItem && uninstall(confirmItem)} oncancel={() => (confirmItem = null)} />
{/if}
{#if confirmOff}
  <ConfirmDialog title={t('content.disableTitle', { name: confirmOff.title })} body={t('content.disableBody')} items={confirmOff.requiredBy.map(title)} confirmLabel={t('content.disable')}
    onconfirm={() => { const it = confirmOff; confirmOff = null; if (it) toggle(it, false); }} oncancel={() => (confirmOff = null)} />
{/if}
{#if blocked}
  <ConfirmDialog infoOnly title={t('content.blockedTitle', { name: blocked.item.title })} body={t('content.blockedBody')} items={blocked.dependents.map(title)} confirmLabel="" onconfirm={() => {}} oncancel={() => (blocked = null)} />
{/if}

<style>
  .ct { height: 100%; display: flex; flex-direction: column; gap: 10px; min-height: 0; }
  .bar { display: flex; flex-wrap: wrap; gap: 8px; align-items: center; }
  .search { position: relative; flex: 1; min-width: 160px; }
  .search :global(svg) { position: absolute; left: 12px; top: 10px; color: var(--paper-mute); pointer-events: none; }
  .search .field { width: 100%; padding-left: 36px; }
  .note { display: flex; align-items: center; gap: 10px; padding: 6px 8px 6px 14px; background: rgba(255, 173, 50, 0.08); border: 1px solid rgba(255, 173, 50, 0.3); border-radius: var(--r-m); color: var(--mango); }
  .note span { flex: 1; color: var(--paper); }
  .list { flex: 1; border: 1px solid var(--line); border-radius: var(--r-m); overflow: hidden; min-height: 0; background: var(--ink-1); }
  .item { display: grid; grid-template-columns: 40px minmax(0, 1fr) auto 44px 36px; gap: 12px; align-items: center; height: 100%; padding: 0 10px 0 12px; border-bottom: 1px solid var(--line-soft); }
  .item:hover { background: var(--ink-2); }
  .item.off .nm, .item.off .ver { opacity: 0.5; }
  .main { min-width: 0; display: grid; gap: 3px; }
  .l1 { display: flex; align-items: center; gap: 8px; min-width: 0; }
  .nm { font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .ver { color: var(--paper-mute); font-size: 12.5px; white-space: nowrap; }
  .tag { font-size: 11.5px; padding: 1px 6px; border-radius: var(--r-s); background: var(--ink-4); color: var(--paper-dim); white-space: nowrap; }
  .tag.prov { border: 1px solid var(--line); background: transparent; }
  .tag.dep { background: rgba(111, 168, 214, 0.16); color: #9cc4e6; }
  .l2 { display: flex; align-items: center; gap: 6px; min-width: 0; color: var(--paper-mute); font-size: 12.5px; overflow: hidden; }
  .rb { color: var(--paper-mute); }
  .chip.sm { height: 20px; padding: 0 7px; font-size: 12px; white-space: nowrap; }
  .fn { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .upd { height: 28px; padding: 0 10px; border-color: var(--mango); color: var(--mango); font-size: 13px; }
  .ic { width: 34px; height: 34px; padding: 0; justify-content: center; }
  .ic:hover:not(:disabled) { color: var(--tomato); }
  .skel { list-style: none; margin: 0; padding: 0; display: grid; gap: 1px; }
  .skel li { display: flex; gap: 12px; align-items: center; height: 62px; }
  .skel .img, .skel .bar1 { background: var(--ink-2); border-radius: var(--r-m); animation: sh 1.4s ease-in-out infinite; }
  .skel .img { width: 40px; height: 40px; }
  .skel .bar1 { width: 260px; height: 14px; }
  @keyframes sh { 50% { opacity: 0.5; } }
</style>
