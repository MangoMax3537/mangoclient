<script lang="ts">
  import HouseIcon from 'phosphor-svelte/lib/HouseIcon';
  import BooksIcon from 'phosphor-svelte/lib/BooksIcon';
  import CompassIcon from 'phosphor-svelte/lib/CompassIcon';
  import HardDrivesIcon from 'phosphor-svelte/lib/HardDrivesIcon';
  import GearIcon from 'phosphor-svelte/lib/GearIcon';
  import Tile from './Tile.svelte';
  import Accounts from './Accounts.svelte';
  import { actions, app, isBusy, nav, selectedAccount, selectedProfile, t } from '../lib/stores.svelte';

  const recent = $derived([...app.profiles].sort((a, b) => (b.lastPlayed ?? 0) - (a.lastPlayed ?? 0)).slice(0, 5));
  const acc = $derived(selectedAccount());
</script>

<nav class="rail" aria-label={t('nav.main')}>
  <ul class="top">
    <li><button type="button" class:on={nav.view === 'home'} aria-current={nav.view === 'home' ? 'page' : undefined} title={t('nav.home')} aria-label={t('nav.home')} onclick={() => (nav.view = 'home')}><HouseIcon weight="bold" size={22} aria-hidden="true" /></button></li>
    <li><button type="button" class:on={nav.view === 'library'} aria-current={nav.view === 'library' ? 'page' : undefined} title={t('nav.library')} aria-label={t('nav.library')} onclick={() => (nav.view = 'library')}><BooksIcon weight="bold" size={22} aria-hidden="true" /></button></li>
    <li><button type="button" disabled title="{t('nav.discover')}: {t('nav.soon')}" aria-label="{t('nav.discover')}, {t('nav.soon')}"><CompassIcon weight="bold" size={22} aria-hidden="true" /></button></li>
    <li><button type="button" disabled title="{t('nav.servers')}: {t('nav.soon')}" aria-label="{t('nav.servers')}, {t('nav.soon')}"><HardDrivesIcon weight="bold" size={22} aria-hidden="true" /></button></li>
  </ul>

  <ul class="recent" aria-label={t('nav.recent')}>
    {#each recent as p (p.id)}
      <li>
        <button type="button" class="prof" class:sel={p.id === selectedProfile()?.id} class:busy={isBusy(p.id)} title={p.name} aria-label={p.name}
          onclick={() => { actions.selectProfile(p.id); nav.view = 'home'; }}>
          <Tile kind="profile" id={p.id} src={p.coverUrl} size={40} radius={6} />
        </button>
      </li>
    {/each}
  </ul>

  <ul class="bottom">
    <li class="acc">
      <button type="button" aria-haspopup="dialog" aria-expanded={nav.accounts} title={t('nav.accounts')} aria-label="{t('nav.accounts')}: {acc?.name ?? t('nav.noAccount')}" onclick={(e) => { e.stopPropagation(); nav.accounts = !nav.accounts; }}>
        <Tile kind="account" id={acc?.uuid ?? ''} accountType={acc?.type} size={36} radius={6} />
      </button>
      {#if nav.accounts}<Accounts />{/if}
    </li>
    <li><button type="button" class:on={nav.view === 'settings'} aria-current={nav.view === 'settings' ? 'page' : undefined} title={t('nav.settings')} aria-label={t('nav.settings')} onclick={() => (nav.view = 'settings')}><GearIcon weight="bold" size={22} aria-hidden="true" /></button></li>
  </ul>
</nav>

<style>
  .rail { width: 72px; flex: none; display: flex; flex-direction: column; align-items: center; padding: 12px 0; gap: 14px; background: var(--ink-1); border-right: 1px solid var(--line-soft); }
  ul { list-style: none; margin: 0; padding: 0; display: grid; gap: 6px; justify-items: center; }
  .recent { padding-top: 14px; border-top: 1px solid var(--line); width: 40px; }
  .bottom { margin-top: auto; }
  li { position: relative; }
  button { width: 44px; height: 44px; display: grid; place-items: center; border-radius: var(--r-m); color: var(--paper-dim); transition: background var(--dur) var(--ease), color var(--dur) var(--ease); }
  button:hover:not(:disabled) { background: var(--ink-3); color: var(--paper); }
  button.on { color: var(--mango); background: var(--ink-3); }
  button.on::before { content: ''; position: absolute; left: -14px; top: 10px; bottom: 10px; width: 3px; background: var(--mango); }
  button:disabled { color: var(--ink-4); }
  .prof { width: 44px; height: 44px; border: 2px solid transparent; }
  .prof.sel { border-color: var(--mango); }
  .prof.busy { animation: pulse 1.4s ease-in-out infinite; }
  @keyframes pulse { 50% { opacity: 0.55; } }
</style>
