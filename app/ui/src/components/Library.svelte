<script lang="ts">
  import MagnifyingGlassIcon from 'phosphor-svelte/lib/MagnifyingGlassIcon';
  import SquaresFourIcon from 'phosphor-svelte/lib/SquaresFourIcon';
  import ListBulletsIcon from 'phosphor-svelte/lib/ListBulletsIcon';
  import PlusIcon from 'phosphor-svelte/lib/PlusIcon';
  import Tile from './Tile.svelte';
  import { fmtDuration } from '../lib/format';
  import { actions, app, nav, t, tp } from '../lib/stores.svelte';

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
  const open = (id: string) => { actions.selectProfile(id); nav.view = 'home'; };
</script>

<div class="lib">
  <header>
    <div>
      <h1>{t('library.title')}</h1>
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
      <button type="button" class:on={mode === 'grid'} aria-pressed={mode === 'grid'} aria-label={t('library.grid')} title={t('library.grid')} onclick={() => (mode = 'grid')}><SquaresFourIcon weight="bold" size={18} aria-hidden="true" /></button>
      <button type="button" class:on={mode === 'list'} aria-pressed={mode === 'list'} aria-label={t('library.list')} title={t('library.list')} onclick={() => (mode = 'list')}><ListBulletsIcon weight="bold" size={18} aria-hidden="true" /></button>
    </div>
  </div>

  {#if items.length === 0}
    <p class="hint">{t('library.empty')}</p>
  {:else if mode === 'grid'}
    <ul class="grid">
      {#each items as p (p.id)}
        <li>
          <button class="card" type="button" onclick={() => open(p.id)} style:--tint={p.color ?? 'var(--line)'}>
            <Tile kind="profile" id={p.id} src={p.coverUrl} size={96} radius={0} />
            <span class="info">
              <b>{p.name}</b>
              <span class="mono sub">{p.mcVersion} &middot; {p.loader === 'vanilla' ? t('profile.vanilla') : p.loader}</span>
              <span class="mono sub dim">{fmtDuration(p.playTimeMs)}</span>
            </span>
          </button>
        </li>
      {/each}
    </ul>
  {:else}
    <ul class="rows">
      {#each items as p (p.id)}
        <li>
          <button class="row" type="button" onclick={() => open(p.id)}>
            <Tile kind="profile" id={p.id} src={p.coverUrl} size={36} radius={4} />
            <b class="nm">{p.name}</b>
            <span class="mono">{p.mcVersion}</span>
            <span>{p.loader === 'vanilla' ? t('profile.vanilla') : p.loader}</span>
            <span class="mono r">{fmtDuration(p.playTimeMs)}</span>
          </button>
        </li>
      {/each}
    </ul>
  {/if}
</div>

<style>
  .lib { padding: 20px 24px; height: 100%; overflow: auto; display: grid; gap: 16px; align-content: start; }
  header { display: flex; justify-content: space-between; align-items: flex-end; }
  h1 { font-size: 34px; }
  .bar { display: flex; gap: 12px; align-items: center; flex-wrap: wrap; }
  .search { position: relative; flex: 1; min-width: 220px; display: block; }
  .search :global(svg) { position: absolute; left: 12px; top: 10px; color: var(--paper-mute); pointer-events: none; }
  .search .field { width: 100%; padding-left: 36px; }
  .sort { display: flex; align-items: center; gap: 8px; }
  .seg { display: flex; border: 1px solid var(--line); border-radius: var(--r-m); overflow: hidden; }
  .seg button { width: 38px; height: 34px; display: grid; place-items: center; color: var(--paper-dim); }
  .seg button.on { background: var(--ink-4); color: var(--mango); }
  ul { list-style: none; margin: 0; padding: 0; }
  .grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(280px, 1fr)); gap: 12px; }
  .card { display: flex; width: 100%; text-align: left; background: var(--ink-2); border: 1px solid var(--line); border-left: 3px solid var(--tint); border-radius: var(--r-m); overflow: hidden; transition: transform var(--dur) var(--ease), background var(--dur) var(--ease); }
  .card:hover { background: var(--ink-3); transform: translateY(-2px); }
  .info { display: grid; align-content: center; gap: 2px; padding: 10px 14px; min-width: 0; }
  .info b { font: 700 17px var(--font-display); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .sub { font-size: 12.5px; color: var(--paper-dim); }
  .dim { color: var(--paper-mute); }
  .rows { border: 1px solid var(--line); border-radius: var(--r-m); overflow: hidden; }
  .row { display: grid; grid-template-columns: 36px 1fr 90px 100px 120px; gap: 14px; align-items: center; width: 100%; padding: 6px 12px; text-align: left; border-bottom: 1px solid var(--line-soft); }
  .row:hover { background: var(--ink-2); }
  .r { text-align: right; }
</style>
