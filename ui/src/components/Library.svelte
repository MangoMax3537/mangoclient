<script lang="ts">
  import MagnifyingGlassIcon from 'phosphor-svelte/lib/MagnifyingGlassIcon';
  import SquaresFourIcon from 'phosphor-svelte/lib/SquaresFourIcon';
  import ListBulletsIcon from 'phosphor-svelte/lib/ListBulletsIcon';
  import PlusIcon from 'phosphor-svelte/lib/PlusIcon';
  import Tile from './Tile.svelte';
  import EmptyState from './EmptyState.svelte';
  import XIcon from 'phosphor-svelte/lib/XIcon';
  import ProhibitIcon from 'phosphor-svelte/lib/ProhibitIcon';
  import { tip } from '../lib/tooltip';
  import { fmtDuration, fmtRelative } from '../lib/format';
  import { app, lang, openProfile, nav, t, tp } from '../lib/stores.svelte';

  let query = $state('');
  let sort = $state<'recent' | 'name' | 'playtime'>('recent');
  let mode = $state<'grid' | 'list'>('grid');

  const items = $derived.by(() => {
    const q = query.trim().toLowerCase();
    const l = app.profiles.filter((p) => !q || p.name.toLowerCase().includes(q) || p.mcVersion.includes(q) || p.loader.includes(q));
    return l.sort((a, b) =>
      sort === 'name' ? a.name.localeCompare(b.name)
        : sort === 'playtime' ? b.playTimeMs - a.playTimeMs
        : (b.lastPlayed ?? 0) - (a.lastPlayed ?? 0));
  });
  const open = (id: string) => openProfile(id);
</script>

<div class="lib">
  <header>
    <div>
      <h1 class="display">{t('library.title')}</h1>
      <div class="hint mono">{tp('library.count', app.profiles.length)}</div>
    </div>
    <button class="btn accent" type="button" onclick={() => (nav.newProfile = true)}><PlusIcon weight="bold" size={16} aria-hidden="true" />{t('library.new')}</button>
  </header>

  <div class="bar">
    <label class="search">
      <MagnifyingGlassIcon weight="bold" size={16} aria-hidden="true" />
      <span class="sr-only">{t('library.search')}</span>
      <input class="field" type="search" placeholder={t('library.search')} bind:value={query} />
    </label>
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
      <button class="btn" type="button" onclick={() => (query = '')}><XIcon weight="bold" size={14} aria-hidden="true" />{t('library.clearSearch')}</button>
    </EmptyState>
  {:else if mode === 'grid'}
    <ul class="grid">
      {#each items as p (p.id)}
        <li>
          <button class="card notch" style:--n="16px" type="button" onclick={() => open(p.id)} style:--tint={p.color ?? 'var(--accent-fill)'}>
            <span class="cover"><Tile kind="profile" id={p.id} src={p.coverUrl} tint={p.color} fill radius={0} /><span class="bar3"></span>{#if p.pendingBlocked > 0}<span class="miss" title={tp('blocked.badge', p.pendingBlocked)}><ProhibitIcon weight="bold" size={12} aria-hidden="true" />{tp('blocked.badge', p.pendingBlocked)}</span>{/if}</span>
            <span class="info">
              <b>{p.name}</b>
              <span class="mono sub">{p.mcVersion}<span class="sep">/</span>{p.loader === 'vanilla' ? t('profile.vanilla') : p.loader}{#if p.modCount}<span class="sep">/</span>{tp('profile.mods', p.modCount)}{/if}</span>
              <span class="mono sub dim">{fmtDuration(p.playTimeMs)}<span class="sep">/</span>{fmtRelative(p.lastPlayed, lang()) ?? t('home.never')}</span>
            </span>
          </button>
        </li>
      {/each}
    </ul>
  {:else}
    <div class="rows">
      <div class="row head" aria-hidden="true"><span></span><span>{t('col.name')}</span><span>{t('col.version')}</span><span>{t('col.loader')}</span><span class="r">{t('col.last')}</span><span class="r">{t('col.time')}</span></div>
      <ul>
        {#each items as p (p.id)}
          <li>
            <button class="row" type="button" onclick={() => open(p.id)}>
              <Tile kind="profile" id={p.id} src={p.coverUrl} tint={p.color} size={36} notch={6} />
              <b class="nm">{p.name}{#if p.pendingBlocked > 0}<span class="miss inl" title={tp('blocked.badge', p.pendingBlocked)}><ProhibitIcon weight="bold" size={12} aria-hidden="true" />{tp('blocked.badge', p.pendingBlocked)}</span>{/if}</b>
              <span class="mono">{p.mcVersion}</span>
              <span>{p.loader === 'vanilla' ? t('profile.vanilla') : p.loader}</span>
              <span class="mono r dim">{fmtRelative(p.lastPlayed, lang()) ?? t('home.never')}</span>
              <span class="mono r">{fmtDuration(p.playTimeMs)}</span>
            </button>
          </li>
        {/each}
      </ul>
    </div>
  {/if}
</div>

<style>
  .lib { padding: var(--s-5) 32px 32px; height: 100%; overflow: auto; display: grid; gap: var(--s-4); align-content: start; }
  header { display: flex; justify-content: space-between; align-items: flex-end; margin-bottom: var(--s-2); }
  h1 { font-size: var(--fs-3xl); }
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
  .card { display: grid; width: 100%; text-align: left; background: var(--ink-2); transition: transform var(--dur) var(--ease); }
  .card:hover { transform: translateY(-2px); }
  .card:hover .info b { color: var(--mango); }
  .cover { position: relative; display: block; aspect-ratio: 16 / 10; }
  .miss { position: absolute; right: 8px; top: 8px; display: inline-flex; align-items: center; gap: 4px; height: 22px; padding: 0 8px; background: var(--accent-fill); color: var(--on-fill); font-size: var(--fs-xs); font-weight: 700; border-radius: var(--r-s); }
  .miss.inl { position: static; margin-left: 10px; height: 18px; padding: 0 6px; vertical-align: 2px; font-family: var(--font-ui); }
  .bar3 { position: absolute; left: 0; bottom: 0; width: 48px; height: 3px; background: var(--tint); }
  .info { display: grid; gap: 2px; padding: 12px 14px 14px; min-width: 0; }
  .info b { font: 700 var(--fs-xl)/1.15 var(--font-display); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; transition: color var(--dur) var(--ease); }
  .sub { font-size: var(--fs-sm); color: var(--paper-dim); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .dim { color: var(--paper-mute); }
  .sep { margin: 0 6px; opacity: 0.5; }
  .rows { border-top: 1px solid var(--line); }
  .row { display: grid; grid-template-columns: 36px minmax(0, 1fr) 80px 100px 130px 120px; gap: var(--s-4); align-items: center; width: 100%; padding: 8px 8px; text-align: left; border-bottom: 1px solid var(--line-soft); }
  button.row { transition: background var(--dur) var(--ease); }
  button.row:hover { background: var(--ink-2); }
  button.row:focus-visible { outline-offset: -2px; }
  .row.head { padding-block: 8px; font-size: var(--fs-xs); letter-spacing: 0.1em; text-transform: uppercase; color: var(--paper-mute); font-weight: 600; border-bottom: 1px solid var(--line); }
  .nm { font: 600 var(--fs-lg) var(--font-display); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .r { text-align: right; }
</style>
