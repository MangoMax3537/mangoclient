<script lang="ts">
  import MagnifyingGlassIcon from 'phosphor-svelte/lib/MagnifyingGlassIcon';
  import DownloadSimpleIcon from 'phosphor-svelte/lib/DownloadSimpleIcon';
  import HeartIcon from 'phosphor-svelte/lib/HeartIcon';
  import CheckIcon from 'phosphor-svelte/lib/CheckIcon';
  import ArrowCircleUpIcon from 'phosphor-svelte/lib/ArrowCircleUpIcon';
  import PlusIcon from 'phosphor-svelte/lib/PlusIcon';
  import CircleNotchIcon from 'phosphor-svelte/lib/CircleNotchIcon';
  import XIcon from 'phosphor-svelte/lib/XIcon';
  import ArrowLeftIcon from 'phosphor-svelte/lib/ArrowLeftIcon';
  import Tile from './Tile.svelte';
  import InstanceIcon from './InstanceIcon.svelte';
  import Seg from './Seg.svelte';
  import SearchResultSkeleton from './skeleton/SearchResultSkeleton.svelte';
  import LoadError from './LoadError.svelte';
  import { BusyGate, createLoadState } from '../lib/loadState.svelte';
  import type { SearchResult } from '../lib/types';
  import EmptyState from './EmptyState.svelte';
  import Lazy from './Lazy.svelte';
  import { dialogs } from '../lib/lazy';
  import { fmtCompact } from '../lib/format';
  import type { Key } from '../lib/i18n';
  import { app, errorText, getApi, installs, lang, nav, openProfile, startInstall, t, tp, tabForType, type FlowProject } from '../lib/stores.svelte';
  import type { ContentItem, Loader, ProjectHit, ProjectType, SearchSort, VersionManifest } from '../lib/types';

  const PAGE = 20;
  const TYPES: ProjectType[] = ['mod', 'modpack', 'resourcepack', 'shader'];
  const LOADERS: Loader[] = ['fabric', 'quilt', 'forge', 'neoforge'];
  const SORTS: SearchSort[] = ['relevance', 'downloads', 'follows', 'newest', 'updated'];

  let query = $state('');
  let loader = $state<Loader | ''>('');
  let mcVersion = $state('');
  let sort = $state<SearchSort>('relevance');
  // One load state for the search: the first page loads with the skeleton, "load more" refreshes silently (old rows stay, skeleton rows are appended).
  const search = createLoadState<SearchResult>({ isEmpty: () => false });
  const hits = $derived(search.data?.hits ?? []);
  const total = $derived(search.data?.total ?? 0);
  const loading = $derived(search.phase === 'loading' || search.phase === 'idle');
  const more = $derived(search.refreshing);
  const error = $derived(search.phase === 'error' ? { code: search.error?.code ?? 'unknown', text: errorText(search.error) } : null);
  const gate = new BusyGate();
  const moreGate = new BusyGate();
  $effect(() => { gate.update(loading); return () => gate.destroy(); });
  $effect(() => { moreGate.update(more); return () => moreGate.destroy(); });
  let manifest = $state<VersionManifest | null>(null);
  let scopeItems = $state<Record<string, ContentItem>>({});
  let drawer = $state<ProjectHit | null>(null);

  const type = $derived(nav.discoverType);
  const scope = $derived(app.profiles.find((p) => p.id === nav.discoverProfile) ?? null);
  const isPack = $derived(type === 'modpack');
  // Data packs are not browsed on their own tab bar entry unless an instance tab sent the user here for them.
  const typeOpts = $derived((type === 'datapack' ? [...TYPES, 'datapack' as ProjectType] : TYPES).map((v) => ({ value: v, label: t(`discover.type.${v}` as Key) })));
  const versions = $derived((manifest?.versions ?? []).filter((v) => v.type === 'release').slice(0, 40));

  // Filters follow the scope instance (and are free when there is none).
  $effect(() => {
    const p = scope;
    loader = p && p.loader !== 'vanilla' && (type === 'mod' || type === 'modpack') ? p.loader : '';
    mcVersion = p && type !== 'modpack' ? p.mcVersion : '';
  });
  $effect(() => { getApi().versionsManifest().then((m) => (manifest = m), () => {}); });
  $effect(() => {
    const id = scope?.id;
    scopeItems = {};
    if (!id) return;
    let dead = false;
    getApi().contentList({ profileId: id }).then((l) => { if (!dead) scopeItems = Object.fromEntries(l.map((i) => [i.projectId, i])); }, () => {});
    return () => { dead = true; };
  });

  function run(append: boolean) {
    const offset = append ? hits.length : 0;
    const prev = append ? hits : [];
    search.run(async (signal) => {
      void signal;
      const r = await getApi().searchProjects({
        params: {
          query: query.trim(), projectType: type, sort, offset, limit: PAGE,
          ...(loader && (type === 'mod' || type === 'modpack') ? { loader } : {}), ...(mcVersion ? { mcVersion } : {}),
        },
      });
      return { ...r, hits: [...prev, ...r.hits] };
    }, { silent: append });
  }
  $effect(() => {
    void type; void query; void loader; void mcVersion; void sort;
    const h = setTimeout(() => run(false), 220);
    return () => { clearTimeout(h); };
  });

  type St = 'install' | 'installing' | 'installed' | 'update';
  const keyOf = (h: ProjectHit) => h.projectId;
  function stateOf(h: ProjectHit): St {
    if (installs[h.projectId]) return 'installing';
    if (h.type === 'modpack' || !scope) return 'install';
    const it = scopeItems[keyOf(h)];
    if (!it) return h.installedIn.includes(scope.id) ? 'installed' : 'install';
    return it.updateAvailable ? 'update' : 'installed';
  }
  const pct = (id: string) => { const e = installs[id]; return e && e.total ? Math.round((e.done / e.total) * 100) : 0; };
  const asProject = (h: ProjectHit): FlowProject => ({
    projectId: h.projectId, title: h.title, type: h.type, iconUrl: h.iconUrl, loaders: h.loaders, gameVersions: h.gameVersions,
  });
  function install(h: ProjectHit, versionId?: string) {
    drawer = null;
    if (h.type === 'modpack') startInstall({ kind: 'modpack', project: asProject(h), versionId });
    else startInstall({ kind: 'content', project: asProject(h), versionId, scopeProfileId: scope?.id ?? null });
  }
  function resetFilters() { query = ''; loader = ''; mcVersion = ''; }
  const filtered = $derived(!!query || !!loader || !!mcVersion);
</script>

<div class="dc">
  <header>
    <div>
      <h1 class="display">{t('nav.discover')}</h1>
      <div class="hint">{t('discover.lead')}</div>
    </div>
  </header>

  {#if scope && !isPack}
    <div class="scope">
      <InstanceIcon profile={scope} size={40} notch={7} />
      <div class="who"><span class="eyebrow">{t('discover.installingTo')}</span><b>{scope.name}</b>
        <span class="mono hint">{scope.mcVersion} / {scope.loader === 'vanilla' ? t('profile.vanilla') : scope.loader}</span></div>
      <button class="btn ghost small" type="button" onclick={() => openProfile(scope.id, tabForType(type))}><ArrowLeftIcon weight="bold" size={14} aria-hidden="true" />{t('discover.backToProfile')}</button>
      <button class="btn small" type="button" onclick={() => (nav.discoverProfile = null)}><XIcon weight="bold" size={14} aria-hidden="true" />{t('discover.allInstances')}</button>
    </div>
  {/if}

  <div class="tabs"><Seg role="tablist" idPrefix="dtype" options={typeOpts} value={type} label={t('discover.typeLabel')} onchange={(v) => (nav.discoverType = v as ProjectType)} /></div>

  <div class="filters">
    <label class="search">
      <MagnifyingGlassIcon weight="bold" size={16} aria-hidden="true" />
      <span class="sr-only">{t('discover.search')}</span>
      <input class="field" type="search" data-search placeholder={t('discover.searchIn')} bind:value={query} />
    </label>
    {#if type === 'mod' || type === 'modpack'}
      <label class="f"><span class="hint">{t('new.loader')}</span>
        <select class="field" bind:value={loader}><option value="">{t('discover.any')}</option>{#each LOADERS as l (l)}<option value={l}>{l}</option>{/each}</select>
      </label>
    {/if}
    <label class="f"><span class="hint">{t('col.version')}</span>
      <select class="field mono" bind:value={mcVersion}><option value="">{t('discover.any')}</option>{#each versions as v (v.id)}<option value={v.id}>{v.id}</option>{/each}</select>
    </label>
    <label class="f"><span class="hint">{t('library.sort')}</span>
      <select class="field" bind:value={sort}>{#each SORTS as s (s)}<option value={s}>{t(`discover.sort.${s}` as Key)}</option>{/each}</select>
    </label>
  </div>

  <div class="results" aria-busy={loading}>
    {#if gate.show}
      <SearchResultSkeleton count={6} />
    {:else if loading && hits.length === 0}
      <!-- first ~150 ms of a first load: nothing yet, so a fast answer never flashes a skeleton -->
    {:else if error}
      <EmptyState title={t('discover.errTitle')} hint={error.text}>
        <button class="btn" type="button" onclick={() => run(false)}>{t('common.retry')}</button>
      </EmptyState>
    {:else if hits.length === 0}
      <EmptyState title={t('discover.emptyTitle')} hint={t('discover.empty')}>
        {#if filtered}<button class="btn" type="button" onclick={resetFilters}><XIcon weight="bold" size={14} aria-hidden="true" />{t('discover.clearFilters')}</button>{/if}
      </EmptyState>
    {:else}
      <p class="hint mono count" aria-live="polite">{t('discover.results', { shown: hits.length, total })}</p>
      <ul class="list">
        {#each hits as h (h.projectId)}
          {@const st = stateOf(h)}
          <li class="row nb">
            <Tile kind="mod" id={h.projectId} src={h.iconUrl} size={56} notch={9} />
            <div class="main">
              <div class="l1"><button class="ttl" type="button" onclick={() => (drawer = h)}>{h.title}</button><span class="by">{t('discover.by', { author: h.author })}</span></div>
              <p class="desc">{h.description}</p>
              <div class="cats">{#each h.categories.slice(0, 3) as c (c)}<span class="tag">{c}</span>{/each}
                {#if !scope && h.installedIn.length && h.type !== 'modpack'}<span class="tag inst">{tp('discover.installedIn', h.installedIn.length)}</span>{/if}</div>
            </div>
            <div class="stats mono">
              <span><DownloadSimpleIcon weight="bold" size={14} aria-hidden="true" /><span class="sr-only">{t('discover.downloads')}</span>{fmtCompact(h.downloads, lang())}</span>
              <span><HeartIcon weight="bold" size={14} aria-hidden="true" /><span class="sr-only">{t('discover.follows')}</span>{fmtCompact(h.follows, lang())}</span>
            </div>
            <button class="btn ib" class:accent={st === 'install'} class:done={st === 'installed'} class:upd={st === 'update'} type="button"
              disabled={st === 'installing' || st === 'installed'} onclick={() => install(h)} aria-label="{isPack && st === 'install' ? t('discover.btn.pack') : t(`discover.btn.${st}` as Key)}: {h.title}">
              {#if st === 'installing'}<span class="spin"><CircleNotchIcon weight="bold" size={14} aria-hidden="true" /></span>{pct(h.projectId)}%
              {:else if st === 'installed'}<CheckIcon weight="bold" size={14} aria-hidden="true" />{t('discover.btn.installed')}
              {:else if st === 'update'}<ArrowCircleUpIcon weight="bold" size={14} aria-hidden="true" />{t('discover.btn.update')}
              {:else}<PlusIcon weight="bold" size={14} aria-hidden="true" />{isPack ? t('discover.btn.pack') : t('discover.btn.install')}{/if}
            </button>
          </li>
        {/each}
      </ul>
      {#if moreGate.show}<SearchResultSkeleton count={3} top={false} />{/if}
      {#if search.refreshError}<LoadError compact error={search.refreshError} onretry={() => run(true)} />{/if}
      {#if hits.length < total}
        <div class="morebar"><button class="btn" type="button" disabled={more} onclick={() => run(true)}>{more ? t('common.loading') : t('discover.loadMore')}</button></div>
      {/if}
    {/if}
  </div>
</div>

{#if drawer}
  <Lazy load={dialogs.projectDrawer} props={{ hit: drawer, scope, installed: scopeItems[keyOf(drawer)] ?? null, busy: stateOf(drawer) === 'installing', progress: pct(drawer.projectId),
    oninstall: (vid: string) => drawer && install(drawer, vid), onclose: () => (drawer = null) }} />
{/if}

<style>
  .dc { height: 100%; overflow: auto; padding: var(--s-5) 32px 32px; display: flex; flex-direction: column; gap: var(--s-3); }
  header { display: flex; justify-content: space-between; align-items: flex-end; gap: var(--s-4); flex-wrap: wrap; }
  h1 { font-size: var(--fs-3xl); }
  .scope { display: flex; align-items: center; gap: 12px; padding: 8px 10px 8px 12px; border: 1px solid var(--line); background: var(--ink-1); }
  .scope .who { flex: 1; display: grid; gap: 0; min-width: 0; }
  .scope b { font: 700 var(--fs-lg) var(--font-display); }
  .filters { display: flex; flex-wrap: wrap; gap: 10px; align-items: flex-end; }
  .search { position: relative; flex: 1; min-width: 220px; }
  .search :global(svg) { position: absolute; left: 12px; top: 10px; color: var(--paper-mute); pointer-events: none; }
  .search .field { width: 100%; padding-left: 36px; }
  .f { display: grid; gap: 2px; font-size: var(--fs-xs); }
  .list { list-style: none; margin: 0; padding: 0; display: grid; gap: 6px; }
  .row { display: grid; grid-template-columns: 56px minmax(0, 1fr) auto 132px; gap: 16px; align-items: center; padding: 12px 14px; --n: var(--notch-s); --fill: var(--ink-1); transition: background var(--dur) var(--ease); }
  .row:hover, .row:focus-within { --fill: var(--ink-2); }
  .row:hover { --line: var(--line-strong, var(--line)); }
  .main { min-width: 0; display: grid; gap: 3px; }
  .l1 { display: flex; align-items: baseline; gap: 8px; min-width: 0; }
  .ttl { font: 700 var(--fs-lg) var(--font-display); text-align: left; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .ttl:hover { color: var(--mango); text-decoration: underline; text-decoration-thickness: 2px; text-underline-offset: 3px; }
  .by { color: var(--paper-mute); font-size: var(--fs-sm); white-space: nowrap; }
  .desc { margin: 0; color: var(--paper-dim); font-size: var(--fs-sm); display: -webkit-box; -webkit-line-clamp: 2; line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden; }
  .cats { display: flex; gap: 6px; flex-wrap: wrap; }
  .tag { font-size: var(--fs-xs); padding: 1px 6px; border: 1px solid var(--line); color: var(--paper-dim); }
  .tag.inst { color: var(--leaf); border-color: color-mix(in srgb, var(--leaf) 45%, transparent); }
  .stats { display: grid; gap: 4px; font-size: var(--fs-sm); color: var(--paper-dim); justify-items: end; }
  .stats span { display: inline-flex; align-items: center; gap: 5px; }
  .ib { justify-content: center; width: 132px; }
  .ib.done { color: var(--leaf); border-color: color-mix(in srgb, var(--leaf) 45%, transparent); background: transparent; opacity: 1; }
  .ib.upd { border-color: var(--mango); color: var(--mango); }
  .count { margin: 0; }
  .morebar { display: flex; justify-content: center; padding: 16px 0 4px; }
  .spin { display: inline-flex; animation: spin 1s linear infinite; }
  @keyframes spin { to { transform: rotate(360deg); } }
  @media (max-width: 1100px) { .stats { display: none; } .row { grid-template-columns: 56px minmax(0, 1fr) 132px; } }
</style>
