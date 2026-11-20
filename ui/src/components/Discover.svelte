<script lang="ts">
  import MagnifyingGlassIcon from 'phosphor-svelte/lib/MagnifyingGlassIcon';
  import DownloadSimpleIcon from 'phosphor-svelte/lib/DownloadSimpleIcon';
  import HeartIcon from 'phosphor-svelte/lib/HeartIcon';
  import CheckIcon from 'phosphor-svelte/lib/CheckIcon';
  import ArrowCircleUpIcon from 'phosphor-svelte/lib/ArrowCircleUpIcon';
  import PlusIcon from 'phosphor-svelte/lib/PlusIcon';
  import CircleNotchIcon from 'phosphor-svelte/lib/CircleNotchIcon';
  import Tile from './Tile.svelte';
  import Seg from './Seg.svelte';
  import ProjectDrawer from './ProjectDrawer.svelte';
  import { fmtCompact } from '../lib/format';
  import type { Key } from '../lib/i18n';
  import { app, errorText, getApi, installs, lang, nav, openProfile, actions, t, toast } from '../lib/stores.svelte';
  import type { ContentItem, Loader, ProjectHit, ProjectType, SearchSort, VersionManifest } from '../lib/types';

  const PAGE = 20;
  const TYPES: ProjectType[] = ['mod', 'modpack', 'resourcepack', 'shader'];
  const LOADERS: Loader[] = ['fabric', 'quilt', 'forge', 'neoforge'];
  const SORTS: SearchSort[] = ['relevance', 'downloads', 'follows', 'newest', 'updated'];

  let query = $state('');
  let loader = $state<Loader | ''>('');
  let mcVersion = $state('');
  let sort = $state<SearchSort>('relevance');
  let hits = $state<ProjectHit[]>([]);
  let total = $state(0);
  let loading = $state(true);
  let more = $state(false);
  let error = $state('');
  let manifest = $state<VersionManifest | null>(null);
  let scopeItems = $state<Record<string, ContentItem>>({});
  let starting = $state<Record<string, boolean>>({});
  let drawer = $state<ProjectHit | null>(null);

  const type = $derived(nav.discoverType);
  const scope = $derived(app.profiles.find((p) => p.id === nav.discoverProfile) ?? null);
  const isPack = $derived(type === 'modpack');
  const typeOpts = $derived(TYPES.map((v) => ({ value: v, label: t(`discover.type.${v}` as Key) })));
  const versions = $derived((manifest?.versions ?? []).filter((v) => v.type === 'release').slice(0, 40));

  // Reset filters from the scope profile whenever it (or the type) changes.
  $effect(() => {
    const p = scope;
    loader = p && p.loader !== 'vanilla' && (type === 'mod' || type === 'modpack') ? p.loader : '';
    mcVersion = p ? p.mcVersion : '';
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

  let seq = 0;
  async function run(append: boolean) {
    const my = ++seq;
    if (append) more = true; else loading = true;
    error = '';
    try {
      const r = await getApi().modrinthSearch({
        params: {
          query: query.trim(), projectType: type, sort, offset: append ? hits.length : 0, limit: PAGE,
          ...(loader && (type === 'mod' || type === 'modpack') ? { loader } : {}), ...(mcVersion ? { mcVersion } : {}),
        },
      });
      if (my !== seq) return;
      hits = append ? [...hits, ...r.hits] : r.hits;
      total = r.total;
    } catch (e) { if (my === seq) error = errorText(e); }
    if (my === seq) { loading = false; more = false; }
  }
  $effect(() => {
    void type; void query; void loader; void mcVersion; void sort;
    const h = setTimeout(() => run(false), 220);
    return () => clearTimeout(h);
  });

  type St = 'install' | 'installing' | 'installed' | 'update';
  function stateOf(h: ProjectHit): St {
    if (installs[h.projectId] || starting[h.projectId]) return 'installing';
    if (h.type === 'modpack') return 'install';
    const it = scopeItems[h.projectId];
    if (!it) return 'install';
    return it.updateAvailable ? 'update' : 'installed';
  }
  const pct = (id: string) => { const e = installs[id]; return e && e.total ? Math.round((e.done / e.total) * 100) : 0; };

  async function install(h: { projectId: string; type: ProjectType; title: string }, versionId?: string) {
    starting[h.projectId] = true;
    try {
      if (h.type === 'modpack') {
        const p = await getApi().modrinthInstallModpack({ projectId: h.projectId, versionId });
        await actions.addProfile(p);
        toast(t('discover.packInstalled', { name: p.name }), 'ok');
        drawer = null;
        openProfile(p.id, 'content');
      } else if (scope) {
        const items = await getApi().modrinthInstall({ profileId: scope.id, projectId: h.projectId, versionId });
        scopeItems = Object.fromEntries(items.map((i) => [i.projectId, i]));
        toast(t('discover.installed', { name: h.title, profile: scope.name }), 'ok');
      }
    } catch (e) { toast(errorText(e), 'error'); }
    delete starting[h.projectId];
  }
</script>

<div class="dc">
  <header>
    <h1>{t('nav.discover')}</h1>
    <div class="scope">
      {#if isPack}
        <span class="hint">{t('discover.newProfile')}</span>
      {:else}
        <label for="dc-scope" class="hint">{t('discover.installingTo')}</label>
        <select id="dc-scope" class="field" value={nav.discoverProfile ?? ''} onchange={(e) => (nav.discoverProfile = e.currentTarget.value || null)}>
          {#each app.profiles as p (p.id)}<option value={p.id}>{p.name} ({p.mcVersion}, {p.loader})</option>{/each}
        </select>
      {/if}
    </div>
  </header>

  <div class="tabs"><Seg role="tablist" idPrefix="dtype" options={typeOpts} value={type} label={t('nav.discover')} onchange={(v) => (nav.discoverType = v as ProjectType)} /></div>

  <div class="filters">
    <label class="search">
      <MagnifyingGlassIcon weight="bold" size={16} aria-hidden="true" />
      <span class="sr-only">{t('discover.search')}</span>
      <input class="field" type="search" placeholder={t('discover.search')} bind:value={query} />
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
    {#if loading}
      <ul class="list">{#each Array(6) as _, i (i)}<li class="row sk"><span class="img"></span><span class="b1"></span></li>{/each}</ul>
    {:else if error}
      <p class="err">{error} <button class="btn" type="button" onclick={() => run(false)}>{t('common.retry')}</button></p>
    {:else if hits.length === 0}
      <p class="hint">{t('discover.empty')}</p>
    {:else}
      <p class="hint mono count" aria-live="polite">{t('discover.results', { shown: hits.length, total })}</p>
      <ul class="list">
        {#each hits as h (h.projectId)}
          {@const st = stateOf(h)}
          <li class="row">
            <Tile kind="mod" id={h.projectId} src={h.iconUrl} size={56} radius={8} />
            <div class="main">
              <div class="l1"><button class="ttl" type="button" onclick={() => (drawer = h)}>{h.title}</button><span class="by">{t('discover.by', { author: h.author })}</span></div>
              <p class="desc">{h.description}</p>
              <div class="cats">{#each h.categories.slice(0, 3) as c (c)}<span class="tag">{c}</span>{/each}</div>
            </div>
            <div class="stats mono">
              <span title={t('discover.downloads')}><DownloadSimpleIcon weight="bold" size={14} aria-hidden="true" /><span class="sr-only">{t('discover.downloads')}</span>{fmtCompact(h.downloads, lang())}</span>
              <span title={t('discover.follows')}><HeartIcon weight="bold" size={14} aria-hidden="true" /><span class="sr-only">{t('discover.follows')}</span>{fmtCompact(h.follows, lang())}</span>
            </div>
            <button class="btn ib" class:accent={st === 'install'} class:done={st === 'installed'} class:upd={st === 'update'} type="button"
              disabled={st === 'installing' || st === 'installed' || (!isPack && !scope)} onclick={() => install(h)} aria-label="{t(`discover.btn.${st}` as Key)}: {h.title}">
              {#if st === 'installing'}<span class="spin"><CircleNotchIcon weight="bold" size={14} aria-hidden="true" /></span>{t('discover.btn.installing')} {pct(h.projectId)}%
              {:else if st === 'installed'}<CheckIcon weight="bold" size={14} aria-hidden="true" />{t('discover.btn.installed')}
              {:else if st === 'update'}<ArrowCircleUpIcon weight="bold" size={14} aria-hidden="true" />{t('discover.btn.update')}
              {:else}<PlusIcon weight="bold" size={14} aria-hidden="true" />{t('discover.btn.install')}{/if}
            </button>
          </li>
        {/each}
      </ul>
      {#if hits.length < total}
        <div class="morebar"><button class="btn" type="button" disabled={more} onclick={() => run(true)}>{more ? t('common.loading') : t('discover.loadMore')}</button></div>
      {/if}
    {/if}
  </div>
</div>

{#if drawer}
  <ProjectDrawer hit={drawer} {scope} installed={scopeItems[drawer.projectId] ?? null} busy={stateOf(drawer) === 'installing'} progress={pct(drawer.projectId)}
    oninstall={(vid) => drawer && install(drawer, vid)} onclose={() => (drawer = null)} />
{/if}

<style>
  .dc { height: 100%; overflow: auto; padding: 18px 24px 24px; display: flex; flex-direction: column; gap: 12px; }
  header { display: flex; justify-content: space-between; align-items: flex-end; gap: 16px; flex-wrap: wrap; }
  h1 { font-size: 34px; }
  .scope { display: flex; align-items: center; gap: 8px; padding: 6px 6px 6px 14px; border: 1px solid var(--line); border-left: 3px solid var(--mango); border-radius: var(--r-m); background: var(--ink-1); min-height: 46px; }
  .scope .field { height: 32px; max-width: 320px; }
  .filters { display: flex; flex-wrap: wrap; gap: 10px; align-items: flex-end; }
  .search { position: relative; flex: 1; min-width: 220px; }
  .search :global(svg) { position: absolute; left: 12px; top: 10px; color: var(--paper-mute); pointer-events: none; }
  .search .field { width: 100%; padding-left: 36px; }
  .f { display: grid; gap: 2px; font-size: 12px; }
  .list { list-style: none; margin: 0; padding: 0; display: grid; border-top: 1px solid var(--line-soft); }
  .row { display: grid; grid-template-columns: 56px minmax(0, 1fr) auto 128px; gap: 14px; align-items: center; padding: 10px 6px; border-bottom: 1px solid var(--line-soft); }
  .row:hover { background: var(--ink-1); }
  .main { min-width: 0; display: grid; gap: 3px; }
  .l1 { display: flex; align-items: baseline; gap: 8px; min-width: 0; }
  .ttl { font: 700 16px var(--font-display); text-align: left; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .ttl:hover { color: var(--mango); text-decoration: underline; }
  .by { color: var(--paper-mute); font-size: 13px; white-space: nowrap; }
  .desc { margin: 0; color: var(--paper-dim); font-size: 13.5px; display: -webkit-box; -webkit-line-clamp: 2; line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden; }
  .cats { display: flex; gap: 6px; }
  .tag { font-size: 11.5px; padding: 1px 6px; border-radius: var(--r-s); background: var(--ink-4); color: var(--paper-dim); }
  .stats { display: grid; gap: 4px; font-size: 13px; color: var(--paper-dim); justify-items: end; }
  .stats span { display: inline-flex; align-items: center; gap: 5px; }
  .ib { justify-content: center; width: 128px; }
  .ib.done { color: var(--leaf); border-color: rgba(134, 184, 107, 0.4); background: transparent; opacity: 1; }
  .ib.upd { border-color: var(--mango); color: var(--mango); }
  .count { margin: 0; }
  .morebar { display: flex; justify-content: center; padding: 16px 0 4px; }
  .sk .img { width: 56px; height: 56px; border-radius: 8px; background: var(--ink-2); animation: sh 1.4s ease-in-out infinite; }
  .sk .b1 { height: 14px; width: 280px; background: var(--ink-2); border-radius: 4px; animation: sh 1.4s ease-in-out infinite; }
  .spin { display: inline-flex; animation: spin 1s linear infinite; }
  @keyframes sh { 50% { opacity: 0.5; } }
  @keyframes spin { to { transform: rotate(360deg); } }
  @media (max-width: 1100px) { .stats { display: none; } .row { grid-template-columns: 56px minmax(0, 1fr) 128px; } }
</style>
