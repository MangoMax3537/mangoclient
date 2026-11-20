<script lang="ts">
  import MagnifyingGlassIcon from 'phosphor-svelte/lib/MagnifyingGlassIcon';
  import SquaresFourIcon from 'phosphor-svelte/lib/SquaresFourIcon';
  import ListBulletsIcon from 'phosphor-svelte/lib/ListBulletsIcon';
  import PlusIcon from 'phosphor-svelte/lib/PlusIcon';
  import InstanceIcon from './InstanceIcon.svelte';
  import EmptyState from './EmptyState.svelte';
  import XIcon from 'phosphor-svelte/lib/XIcon';
  import DotsThreeIcon from 'phosphor-svelte/lib/DotsThreeIcon';
  import PaletteIcon from 'phosphor-svelte/lib/PaletteIcon';
  import ImageIcon from 'phosphor-svelte/lib/ImageIcon';
  import DownloadSimpleIcon from 'phosphor-svelte/lib/DownloadSimpleIcon';
  import CaretDownIcon from 'phosphor-svelte/lib/CaretDownIcon';
  import CaretRightIcon from 'phosphor-svelte/lib/CaretRightIcon';
  import TagIcon from 'phosphor-svelte/lib/TagIcon';
  import FoldersIcon from 'phosphor-svelte/lib/FoldersIcon';
  import { tip } from '../lib/tooltip';
  import { fmtDuration, fmtRelative } from '../lib/format';
  import { filterProfiles, groupSections, knownTags } from '../lib/portability';
  import { writeLibraryLayout } from '../lib/layoutHint';
  import { pickAndImport } from '../lib/portability.svelte';
  import { app, lang, openBackgroundPicker, openIconPicker, openProfile, nav, t, tp } from '../lib/stores.svelte';
  import type { ProfileView } from '../lib/types';

  let query = $state('');
  let sort = $state<'recent' | 'name' | 'playtime'>('recent');
  let mode = $state<'grid' | 'list'>('grid');

  let tag = $state<string | null>(null);
  const COLLAPSE_KEY = 'mango.libraryCollapsed';
  let collapsed = $state<Record<string, boolean>>({});
  try { collapsed = JSON.parse(globalThis.localStorage?.getItem(COLLAPSE_KEY) ?? '{}') ?? {}; } catch { /* storage may be unavailable */ }
  const tagList = $derived(knownTags(app.profiles));
  $effect(() => { if (tag && !tagList.some((x) => x.tag.toLowerCase() === tag?.toLowerCase())) tag = null; });

  const items = $derived.by(() => {
    const l = filterProfiles(app.profiles, query, tag);
    return l.sort((a, b) =>
      sort === 'name' ? a.name.localeCompare(b.name)
        : sort === 'playtime' ? b.playTimeMs - a.playTimeMs
        : (b.lastPlayed ?? 0) - (a.lastPlayed ?? 0));
  });
  const sections = $derived(groupSections(items));
  /** Headers only when at least one instance has a group. */
  const grouped = $derived(sections.some((x) => x.group !== null));
  $effect(() => { writeLibraryLayout({ tags: app.profiles.some((p) => p.tags.length > 0), grouped }); });
  const filtering = $derived(!!query.trim() || !!tag);
  const isCollapsed = (key: string) => !filtering && !!collapsed[key];
  function toggle(key: string) {
    collapsed = { ...collapsed, [key]: !collapsed[key] };
    try { globalThis.localStorage?.setItem(COLLAPSE_KEY, JSON.stringify(collapsed)); } catch { /* ignore */ }
  }
  const open = (id: string) => openProfile(id);
  let menu = $state<string | null>(null);
  function pick(fn: (id: string) => void, id: string) { menu = null; fn(id); }
  function onDoc(e: MouseEvent) { if (menu && !(e.target as Element | null)?.closest('.cm')) menu = null; }
  function onKey(e: KeyboardEvent) { if (menu && e.key === 'Escape') menu = null; }
</script>

<svelte:document onclick={onDoc} onkeydown={onKey} />
<div class="lib">
  <header>
    <div>
      <h1 class="display">{t('library.title')}</h1>
      <div class="hint mono">{tp('library.count', app.profiles.length)}</div>
    </div>
    <div class="hacts">
      <button class="btn" type="button" use:tip={{ text: t('import.buttonHint'), side: 'top' }} onclick={pickAndImport}><DownloadSimpleIcon weight="bold" size={16} aria-hidden="true" />{t('import.button')}</button>
      <button class="btn accent" type="button" onclick={() => (nav.newProfile = true)}><PlusIcon weight="bold" size={16} aria-hidden="true" />{t('library.new')}</button>
    </div>
  </header>

  <div class="bar">
    <label class="search">
      <MagnifyingGlassIcon weight="bold" size={16} aria-hidden="true" />
      <span class="sr-only">{t('library.searchAll')}</span>
      <input class="field" type="search" data-search placeholder={t('library.searchAll')} bind:value={query} />
    </label>
    {#if tagList.length}
      <label class="sort">
        <span class="hint">{t('library.tag')}</span>
        <select class="field" value={tag ?? ''} onchange={(e) => (tag = e.currentTarget.value || null)}>
          <option value="">{t('library.allTags')}</option>
          {#each tagList as k (k.tag)}<option value={k.tag}>{k.tag} ({k.count})</option>{/each}
        </select>
      </label>
    {/if}
    <label class="sort">
      <span class="hint">{t('library.sort')}</span>
      <select class="field" bind:value={sort}>
        <option value="recent">{t('library.sort.recent')}</option>
        <option value="name">{t('library.sort.name')}</option>
        <option value="playtime">{t('library.sort.playtime')}</option>
      </select>
    </label>
    <div class="seg" role="group">
      <button type="button" class:on={mode === 'grid'} aria-pressed={mode === 'grid'} aria-label={t('library.grid')} use:tip={{ text: t('library.grid'), side: 'top' }} onclick={() => (mode = 'grid')}><SquaresFourIcon weight="bold" size={18} aria-hidden="true" /></button>
      <button type="button" class:on={mode === 'list'} aria-pressed={mode === 'list'} aria-label={t('library.list')} use:tip={{ text: t('library.list'), side: 'top' }} onclick={() => (mode = 'list')}><ListBulletsIcon weight="bold" size={18} aria-hidden="true" /></button>
    </div>
  </div>

  {#if app.profiles.length === 0}
    <EmptyState title={t('home.noProfiles')} hint={t('home.noProfilesHint')}>
      <button class="btn accent" type="button" onclick={() => (nav.newProfile = true)}><PlusIcon weight="bold" size={16} aria-hidden="true" />{t('library.new')}</button>
    </EmptyState>
  {:else if items.length === 0}
    <EmptyState title={t('library.empty')} hint={t('library.emptyHint')}>
      <button class="btn" type="button" onclick={() => { query = ''; tag = null; }}><XIcon weight="bold" size={14} aria-hidden="true" />{t('library.clearFilters')}</button>
    </EmptyState>
  {:else}
    {#if mode === 'list'}<div class="row head" aria-hidden="true"><span></span><span>{t('col.name')}</span><span>{t('col.version')}</span><span>{t('col.loader')}</span><span class="r">{t('col.last')}</span><span class="r">{t('col.time')}</span></div>{/if}
    {#each sections as sec (sec.key)}
      <section class="sec" aria-label={sec.group ?? t('group.ungrouped')}>
        {#if grouped}
          <h2 class="gh">
            <button type="button" aria-expanded={!isCollapsed(sec.key)} aria-label={t('library.groupToggle', { group: sec.group ?? t('group.ungrouped') })} onclick={() => toggle(sec.key)}>
              {#if isCollapsed(sec.key)}<CaretRightIcon weight="bold" size={14} aria-hidden="true" />{:else}<CaretDownIcon weight="bold" size={14} aria-hidden="true" />{/if}
              <FoldersIcon weight="bold" size={16} aria-hidden="true" />
              <span class="gn">{sec.group ?? t('group.ungrouped')}</span>
              <span class="mono hint">{sec.items.length}</span>
            </button>
          </h2>
        {/if}
        {#if !isCollapsed(sec.key)}
          {#if mode === 'grid'}
            <ul class="grid">{#each sec.items as p (p.id)}{@render card(p)}{/each}</ul>
          {:else}
            <div class="rows">
              <ul>{#each sec.items as p (p.id)}{@render row(p)}{/each}</ul>
            </div>
          {/if}
        {/if}
      </section>
    {/each}
  {/if}
</div>

{#snippet tagChips(p: ProfileView, max: number)}
  {#if p.tags.length}
    <span class="tags" aria-label={t('library.tagsOn')}>
      {#each p.tags.slice(0, max) as tg (tg)}<span class="tg mono"><TagIcon weight="bold" size={10} aria-hidden="true" />{tg}</span>{/each}
      {#if p.tags.length > max}<span class="tg mono">+{p.tags.length - max}</span>{/if}
    </span>
  {/if}
{/snippet}

{#snippet card(p: ProfileView)}
  <li class="cell">
    <button class="card notch" style:--n="16px" type="button" onclick={() => open(p.id)} style:--tint={p.color ?? 'var(--accent-fill)'}>
      <span class="cover"><InstanceIcon profile={p} fill radius={0} /></span>
      <span class="info">
        <b>{p.name}</b>
        <span class="mono sub">{p.mcVersion}<span class="sep">/</span>{p.loader === 'vanilla' ? t('profile.vanilla') : p.loader}{#if p.modCount}<span class="sep">/</span>{tp('profile.mods', p.modCount)}{/if}</span>
        <span class="mono sub dim">{fmtDuration(p.playTimeMs)}<span class="sep">/</span>{fmtRelative(p.lastPlayed, lang()) ?? t('home.never')}</span>
        {@render tagChips(p, 3)}
      </span>
    </button>
    <div class="cm">
      <button class="kebab" type="button" aria-haspopup="menu" aria-expanded={menu === p.id} aria-label={t('library.menu', { name: p.name })} onclick={() => (menu = menu === p.id ? null : p.id)}><DotsThreeIcon weight="bold" size={18} aria-hidden="true" /></button>
      {#if menu === p.id}
        <ul class="pop menu-list" role="menu" aria-label={t('library.menu', { name: p.name })}>
          <li role="none"><button type="button" role="menuitem" onclick={() => pick(openIconPicker, p.id)}><PaletteIcon weight="bold" size={16} aria-hidden="true" />{t('icons.change')}</button></li>
          <li role="none"><button type="button" role="menuitem" onclick={() => pick(openBackgroundPicker, p.id)}><ImageIcon weight="bold" size={16} aria-hidden="true" />{t('bg.change')}</button></li>
          <li role="none"><button type="button" role="menuitem" onclick={() => pick((id) => openProfile(id, 'settings'), p.id)}><TagIcon weight="bold" size={16} aria-hidden="true" />{t('library.menu.group')}</button></li>
        </ul>
      {/if}
    </div>
  </li>
{/snippet}

{#snippet row(p: ProfileView)}
  <li>
    <button class="row" type="button" onclick={() => open(p.id)}>
      <InstanceIcon profile={p} size={36} notch={6} />
      <span class="nmc"><b class="nm">{p.name}</b>{@render tagChips(p, 4)}</span>
      <span class="mono">{p.mcVersion}</span>
      <span>{p.loader === 'vanilla' ? t('profile.vanilla') : p.loader}</span>
      <span class="mono r dim">{fmtRelative(p.lastPlayed, lang()) ?? t('home.never')}</span>
      <span class="mono r">{fmtDuration(p.playTimeMs)}</span>
    </button>
  </li>
{/snippet}

<style>
  .lib { padding: var(--s-5) 32px 32px; height: 100%; overflow: auto; display: grid; gap: var(--s-4); align-content: start; }
  header { display: flex; justify-content: space-between; align-items: flex-end; margin-bottom: var(--s-2); }
  h1 { font-size: var(--fs-3xl); }
  .hacts { display: flex; gap: var(--s-2); }
  .sec { display: grid; gap: var(--s-3); }
  .gh { margin: 0; font-size: var(--fs-md); }
  .gh button { display: flex; align-items: center; gap: 8px; width: 100%; height: 34px; padding: 0 4px; border-bottom: 1px solid var(--line); color: var(--paper-dim); text-align: left; }
  .gh button:hover { color: var(--paper); }
  .gh .gn { font: 700 var(--fs-lg) var(--font-display); color: var(--paper); }
  .tags { display: flex; flex-wrap: wrap; gap: 4px; margin-top: 4px; }
  .tg { display: inline-flex; align-items: center; gap: 3px; height: 18px; padding: 0 6px; border: 1px solid var(--line); border-radius: var(--r-s); font-size: 11px; color: var(--paper-dim); }
  .nmc { display: grid; min-width: 0; }
  .nmc .tags { margin: 2px 0 0; }
  .bar { display: flex; gap: var(--s-3); align-items: center; flex-wrap: wrap; }
  .search { position: relative; flex: 1; min-width: 220px; display: block; }
  .search :global(svg) { position: absolute; left: 12px; top: 10px; color: var(--paper-mute); pointer-events: none; }
  .search .field { width: 100%; padding-left: 36px; }
  .sort { display: flex; align-items: center; gap: var(--s-2); }
  .seg { display: flex; border: 1px solid var(--line); border-radius: var(--r-m); overflow: hidden; }
  .seg button { width: 38px; height: 34px; display: grid; place-items: center; color: var(--paper-dim); position: relative; }
  .seg button:hover { color: var(--paper); background: var(--ink-3); }
  .seg button.on { background: var(--ink-4); color: var(--paper); }
  .seg button.on::after { content: ''; position: absolute; left: 6px; right: 6px; bottom: 0; height: 2px; background: var(--accent-fill); }
  .seg button:focus-visible { outline-offset: -3px; }
  ul { list-style: none; margin: 0; padding: 0; }
  .grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(232px, 1fr)); gap: var(--s-4); }
  .cell { position: relative; display: grid; }
  .cm { position: absolute; right: 8px; top: 8px; z-index: 3; }
  .kebab { width: 30px; height: 30px; display: grid; place-items: center; border-radius: var(--r-s); background: rgba(11, 12, 14, 0.62); color: #f4f1ea; opacity: 0; transition: opacity var(--dur) var(--ease); }
  .cell:hover .kebab, .cell:focus-within .kebab, .kebab[aria-expanded='true'] { opacity: 1; }
  .kebab:hover { background: rgba(11, 12, 14, 0.85); }
  .cm .pop { position: absolute; right: 0; top: 34px; min-width: 190px; }
  .card { display: grid; grid-template-rows: auto 1fr; width: 100%; text-align: left; background: var(--ink-2); transition: transform var(--dur) var(--ease); }
  .card:hover { transform: translateY(-2px); }
  .card:hover .info b { color: var(--mango); }
  .cover { position: relative; display: block; aspect-ratio: 16 / 10; }
  .info { display: grid; align-content: start; gap: 2px; padding: 12px 14px 14px; min-width: 0; }
  .info b { font: 700 var(--fs-xl)/1.15 var(--font-display); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; transition: color var(--dur) var(--ease); }
  .sub { font-size: var(--fs-sm); color: var(--paper-dim); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .dim { color: var(--paper-mute); }
  .sep { margin: 0 6px; opacity: 0.5; }
  .row { display: grid; grid-template-columns: 36px minmax(0, 1fr) 64px 76px 124px 120px; gap: var(--s-3); align-items: center; width: 100%; padding: 8px 8px; text-align: left; border-bottom: 1px solid var(--line-soft); }
  button.row { transition: background var(--dur) var(--ease); }
  button.row:hover { background: var(--ink-2); }
  button.row:focus-visible { outline-offset: -2px; }
  .row.head { padding-inline: 8px; border-top: 1px solid var(--line); padding-block: 8px; font-size: var(--fs-xs); letter-spacing: 0.1em; text-transform: uppercase; color: var(--paper-mute); font-weight: 600; border-bottom: 1px solid var(--line); }
  .nm { font: 600 var(--fs-lg) var(--font-display); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .r { text-align: right; white-space: nowrap; }
</style>
