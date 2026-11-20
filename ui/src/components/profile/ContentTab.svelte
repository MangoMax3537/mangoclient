<script lang="ts">
  // One content tab (Mods, Shaders, Resource packs, Data packs): the same component, parameterized by `type`. The instance's content list
  // and the update infos are loaded once by ProfilePage (they also feed the tab counts) and passed in.
  import { untrack } from 'svelte';
  import MagnifyingGlassIcon from 'phosphor-svelte/lib/MagnifyingGlassIcon';
  import FolderOpenIcon from 'phosphor-svelte/lib/FolderOpenIcon';
  import TrashIcon from 'phosphor-svelte/lib/TrashIcon';
  import ArrowCircleUpIcon from 'phosphor-svelte/lib/ArrowCircleUpIcon';
  import LinkIcon from 'phosphor-svelte/lib/LinkIcon';
  import PlusIcon from 'phosphor-svelte/lib/PlusIcon';
  import Tile from '../Tile.svelte';
  import Toggle from '../Toggle.svelte';
  import VirtualList from '../VirtualList.svelte';
  import ConfirmDialog from '../ConfirmDialog.svelte';
  import EmptyState from '../EmptyState.svelte';
  import LoadError from '../LoadError.svelte';
  import ListRowSkeleton from '../skeleton/ListRowSkeleton.svelte';
  import type { Key } from '../../lib/i18n';
  import type { LoadState } from '../../lib/loadState.svelte';
  import { app, errorText, getApi, installs, nav, openDiscover, startInstall, t, toast, tp } from '../../lib/stores.svelte';
  import type { ContentItem, ContentType, ProfileView, UpdateInfo } from '../../lib/types';

  interface Props {
    profile: ProfileView;
    type: ContentType;
    /** Everything installed in the instance (all types); `null` until the first answer. */
    items: ContentItem[] | null;
    infos: UpdateInfo[];
    load: Pick<LoadState<ContentItem[]>, 'phase' | 'error' | 'showSkeleton' | 'retry'>;
    onitems: (items: ContentItem[]) => void;
  }
  let { profile, type, items, infos, load, onitems }: Props = $props();

  // A link from the crash panel opens the Mods tab already searching for that mod; the request is used once.
  let query = $state(untrack(() => (type === 'mod' ? nav.contentQuery : null) ?? ''));
  $effect(() => { if (type === 'mod' && nav.contentQuery !== null) { query = nav.contentQuery; nav.contentQuery = null; } });
  let pending = $state<Record<string, boolean>>({});
  let confirmItem = $state<ContentItem | null>(null);
  let blocked = $state<{ item: ContentItem; dependents: string[] } | null>(null);
  let confirmOff = $state<ContentItem | null>(null);
  let working = $state(false);

  const title = (id: string) => items?.find((i) => i.projectId === id)?.title ?? id;
  const mine = $derived((items ?? []).filter((i) => i.type === type));
  const shown = $derived.by(() => {
    const q = query.trim().toLowerCase();
    return mine.filter((i) => !q || i.title.toLowerCase().includes(q) || i.filename.toLowerCase().includes(q)).sort((a, b) => a.title.localeCompare(b.title));
  });
  const myInfos = $derived(infos.filter((u) => u.type === type));
  const updatable = $derived(mine.filter((i) => i.updateAvailable));

  async function toggle(it: ContentItem, enabled: boolean) {
    pending[it.projectId] = true;
    try {
      const r = await getApi().contentToggle({ profileId: profile.id, projectId: it.projectId, enabled });
      onitems(r.items);
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
      else { onitems(r.items); toast(t('content.uninstalled', { name: it.title }), 'ok'); }
    } catch (e) { toast(errorText(e), 'error'); confirmItem = null; }
    working = false;
  }
  function askUninstall(it: ContentItem) {
    if (it.requiredBy.length) uninstall(it); // the backend answers with the blocking dependents
    else confirmItem = it;
  }
  /** Single update: straight to the dependency dialog for this instance (picker skipped), showing what the new version needs. */
  function update(it: ContentItem) {
    const info = infos.find((u) => u.projectId === it.projectId);
    startInstall({
      kind: 'content', update: true, scopeProfileId: profile.id, versionId: info?.versionId,
      project: { projectId: it.projectId, title: it.title, type: it.type, iconUrl: it.iconUrl },
    });
  }
  /** Update all of this type: first a preview of versions and new dependencies, nothing is applied before the confirmation. */
  function updateAll() { startInstall({ kind: 'updates', profileId: profile.id, infos: myInfos }); }
  /** Disabling something other content needs cascades; say so before it happens. */
  function askToggle(it: ContentItem, enabled: boolean) {
    const live = it.requiredBy.filter((id) => items?.find((i) => i.projectId === id)?.enabled);
    if (!enabled && live.length) confirmOff = it; else toggle(it, enabled);
  }
  async function openFolder() {
    try { toast(await getApi().contentOpenFolder({ profileId: profile.id, type }), 'info'); } catch (e) { toast(errorText(e), 'error'); }
  }
  const add = () => openDiscover(profile.id, type);
  const k = (base: string) => `${base}.${type}` as Key;
</script>

<div class="ct">
  <!-- nothing installed: no search, no folder button, no toolbar. The empty state below carries the one action that makes sense. -->
  {#if items === null || mine.length > 0}
  <div class="bar">
    <label class="search">
      <MagnifyingGlassIcon weight="bold" size={16} aria-hidden="true" />
      <span class="sr-only">{t(k('content.search'))}</span>
      <input class="field" type="search" data-search placeholder={t(k('content.search'))} bind:value={query} />
    </label>
    <button class="btn" type="button" onclick={openFolder}><FolderOpenIcon weight="bold" size={16} aria-hidden="true" />{t('content.openFolder')}</button>
    <button class="btn accent" type="button" onclick={add}><PlusIcon weight="bold" size={16} aria-hidden="true" />{t(k('content.add'))}</button>
  </div>
  {/if}

  {#if updatable.length}
    <div class="note" role="status">
      <ArrowCircleUpIcon weight="bold" size={16} aria-hidden="true" />
      <span>{tp(`content.updatesAvailable.${type}`, updatable.length)}</span>
      <button class="btn" type="button" disabled={!myInfos.length} onclick={updateAll}>{t('content.updateAll')}</button>
    </div>
  {/if}

  {#if items === null}
    {#if load.phase === 'error'}<LoadError error={load.error} onretry={() => load.retry()} />
    {:else if load.showSkeleton}<div class="list"><ListRowSkeleton count={8} rowHeight={62} img={40} tail={34} /></div>{/if}
  {:else if mine.length === 0}
    <EmptyState title={t(k('content.emptyTitle'))} hint={t(k('content.empty'))}>
      <button class="btn accent" type="button" onclick={add}><PlusIcon weight="bold" size={16} aria-hidden="true" />{t(k('content.add'))}</button>
      <button class="btn" type="button" onclick={openFolder}><FolderOpenIcon weight="bold" size={16} aria-hidden="true" />{t('content.openFolder')}</button>
    </EmptyState>
  {:else if shown.length === 0}
    <EmptyState title={t('content.noMatch')} hint={t('library.emptyHint')}>
      <button class="btn" type="button" onclick={() => (query = '')}>{t('library.clearSearch')}</button>
    </EmptyState>
  {:else}
    <div class="list">
      <VirtualList separators items={shown} rowHeight={62} key={(i) => i.projectId} label={t(`content.type.${type}` as Key)}>
        {#snippet row(it)}
          {@const inst = installs[it.projectId]}
          <div class="item" class:off={!it.enabled}>
            <Tile kind="mod" id={it.projectId} src={it.iconUrl} size={40} radius={6} />
            <div class="main">
              <div class="l1">
                <b class="nm">{it.title}</b>
                <span class="ver mono">{it.versionNumber ?? '?'}</span>
                {#if it.dependency}<span class="tag dep">{t('content.dependency')}</span>{/if}
                {#if it.local}<span class="tag">{t('content.local')}</span>{/if}
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
              <button class="btn upd" type="button" disabled={!!inst || it.local} onclick={() => update(it)} aria-label={t('content.updateTo', { name: it.title, v: it.updateAvailable })}>
                <ArrowCircleUpIcon weight="bold" size={14} aria-hidden="true" />{inst ? `${Math.round((inst.done / inst.total) * 100)}%` : it.updateAvailable}
              </button>
            {:else}<span></span>{/if}
            <Toggle checked={it.enabled} disabled={pending[it.projectId]} label="{t('content.enabled')}: {it.title}"
              onchange={(on, e) => { if (!on && it.requiredBy.length) e.currentTarget.checked = true; askToggle(it, on); }} />
            <button class="btn ghost ic" type="button" aria-label="{t('content.uninstall')}: {it.title}" title={t('content.uninstall')} onclick={() => askUninstall(it)}><TrashIcon weight="bold" size={16} aria-hidden="true" /></button>
          </div>
        {/snippet}
      </VirtualList>
    </div>
  {/if}
</div>

{#if confirmItem}
  <ConfirmDialog title={t('content.uninstallTitle', { name: confirmItem.title })} body={t('content.uninstallBody', { file: confirmItem.filename })} facts={[{ label: t('col.version'), value: confirmItem.versionNumber ?? '?' }, ...(confirmItem.local ? [{ label: t('content.source'), value: t('content.local') }] : [])]} warn={confirmItem.dependency ? t('content.uninstallDepWarn') : ''} note={app.config?.autoSnapshot ? t('flow.snapshotFirst') : ''} confirmLabel={t('content.uninstall')} danger busy={working}
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
  .item { display: grid; grid-template-columns: 40px minmax(0, 1fr) auto 44px 36px; gap: 12px; align-items: center; height: 100%; padding: 0 10px 0 12px; }
  .item:hover { background: var(--ink-2); }
  .item.off .nm, .item.off .ver { opacity: 0.5; }
  .main { min-width: 0; display: grid; gap: 3px; }
  .l1 { display: flex; align-items: center; gap: 8px; min-width: 0; }
  .nm { font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .ver { color: var(--paper-mute); font-size: 12.5px; white-space: nowrap; }
  .tag { font-size: 11.5px; padding: 1px 6px; border-radius: var(--r-s); background: var(--ink-4); color: var(--paper-dim); white-space: nowrap; }
  .tag.dep { background: rgba(111, 168, 214, 0.16); color: #9cc4e6; }
  .l2 { display: flex; align-items: center; gap: 6px; min-width: 0; color: var(--paper-mute); font-size: 12.5px; overflow: hidden; }
  .rb { color: var(--paper-mute); }
  .chip.sm { height: 20px; padding: 0 7px; font-size: 12px; white-space: nowrap; }
  .fn { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .upd { height: 28px; padding: 0 10px; border-color: var(--mango); color: var(--mango); font-size: 13px; }
  .ic { width: 34px; height: 34px; padding: 0; justify-content: center; }
  .ic:hover:not(:disabled) { color: var(--tomato); }
</style>
