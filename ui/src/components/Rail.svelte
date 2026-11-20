<script lang="ts">
  import HouseIcon from 'phosphor-svelte/lib/HouseIcon';
  import BooksIcon from 'phosphor-svelte/lib/BooksIcon';
  import CompassIcon from 'phosphor-svelte/lib/CompassIcon';
  import HardDrivesIcon from 'phosphor-svelte/lib/HardDrivesIcon';
  import ChartBarIcon from 'phosphor-svelte/lib/ChartBarIcon';
  import GearIcon from 'phosphor-svelte/lib/GearIcon';
  import Tile from './Tile.svelte';
  import InstanceIcon from './InstanceIcon.svelte';
  import Accounts from './Accounts.svelte';
  import { tip } from '../lib/tooltip';
  import { actions, app, isBusy, accountNeedsLogin, keystoreMissing, nav, openDiscover, openProfile, selectedAccount, selectedProfile, t } from '../lib/stores.svelte';

  const recent = $derived([...app.profiles].sort((a, b) => (b.lastPlayed ?? 0) - (a.lastPlayed ?? 0)).slice(0, 5));
  const acc = $derived(selectedAccount());
  // credential-state dot on the account button: ok / not saved / sign-in needed (offline accounts have none)
  const cred = $derived(!acc || acc.type !== 'microsoft' ? null : accountNeedsLogin(acc) ? 'login' : (acc.credentialsLocked || acc.credentialsPersisted === false) ? 'warn' : 'ok');
  const credText = $derived(cred === 'login' ? t('rail.cred.login') : cred === 'warn' ? t('rail.cred.warn') : cred === 'ok' ? t('rail.cred.ok') : '');
  const items = $derived([
    { v: 'home', label: t('nav.home'), icon: HouseIcon, go: () => (nav.view = 'home') },
    { v: 'library', label: t('nav.library'), icon: BooksIcon, go: () => (nav.view = 'library') },
    { v: 'discover', label: t('nav.discover'), icon: CompassIcon, go: () => openDiscover() },
    { v: 'servers', label: t('nav.servers'), icon: HardDrivesIcon, go: () => (nav.view = 'servers') },
    { v: 'stats', label: t('nav.stats'), icon: ChartBarIcon, go: () => (nav.view = 'stats') },
  ] as const);
</script>

<nav class="rail" aria-label={t('nav.main')}>
  <ul class="top">
    {#each items as it (it.v)}
      <li class:on={nav.view === it.v}>
        <button type="button" class="nav" use:tip={it.label} aria-current={nav.view === it.v ? 'page' : undefined} aria-label={it.label} onclick={it.go}>
          <it.icon weight="bold" size={22} aria-hidden="true" />
        </button>
      </li>
    {/each}
  </ul>

  <ul class="recent" aria-label={t('nav.recent')}>
    {#each recent as p (p.id)}
      {@const sel = nav.view === 'profile' ? p.id === nav.profileId : p.id === selectedProfile()?.id}
      <li class:sel class:on={nav.view === 'profile' && p.id === nav.profileId}>
        <button type="button" class="prof" use:tip={p.name} aria-label={p.name} aria-current={sel ? 'true' : undefined}
          onclick={() => { actions.selectProfile(p.id); openProfile(p.id); }}>
          <span class="ring notch" style:--n="9px"><InstanceIcon profile={p} size={40} notch={7} /></span>
        </button>
        {#if isBusy(p.id)}<span class="sq ok pulse" aria-hidden="true"></span><span class="sr-only">{t('home.running')}</span>{/if}
      </li>
    {/each}
  </ul>

  <ul class="bottom">
    <li class="acc">
      <button type="button" class="avatar" aria-haspopup="dialog" aria-expanded={nav.accounts} use:tip={`${acc?.name ?? t('nav.noAccount')}${credText ? ` · ${credText}` : ''}`}
        aria-label="{t('nav.accounts')}: {acc?.name ?? t('nav.noAccount')}{credText ? `. ${credText}` : ''}{keystoreMissing() ? `. ${t('persist.railHint')}` : ''}"
        onclick={(e) => { e.stopPropagation(); nav.accounts = !nav.accounts; }}>
        <span class="ring notch" style:--n="9px"><Tile kind="account" id={acc?.uuid ?? ''} src={acc?.headUrl} size={40} notch={7} /></span>
        {#if cred}<span class="sq cred" class:ok={cred === 'ok'} class:warn={cred === 'warn'} class:bad={cred === 'login'} aria-hidden="true"></span>{/if}
      </button>
      {#if nav.accounts}<Accounts />{/if}
    </li>
    <li class:on={nav.view === 'settings'}>
      <button type="button" class="nav" use:tip={t('nav.settings')} aria-current={nav.view === 'settings' ? 'page' : undefined} aria-label={t('nav.settings')} onclick={() => (nav.view = 'settings')}><GearIcon weight="bold" size={22} aria-hidden="true" /></button>
    </li>
  </ul>
</nav>

<style>
  .rail { width: 72px; flex: none; display: flex; flex-direction: column; align-items: center; padding: 12px 0; gap: 10px; background: var(--ink-1); border-right: 1px solid var(--line-soft); }
  ul { list-style: none; margin: 0; padding: 0; display: grid; gap: 6px; justify-items: center; }
  .recent { padding-top: 12px; border-top: 1px solid var(--line); width: 44px; flex: 1; min-height: 0; overflow: hidden; align-content: start; }
  .bottom { flex: none; }
  li { position: relative; }
  button { position: relative; width: 44px; height: 44px; display: grid; place-items: center; color: var(--paper-dim); transition: background var(--dur) var(--ease), color var(--dur) var(--ease); }
  .nav { clip-path: polygon(0 0, calc(100% - 8px) 0, 100% 8px, 100% 100%, 8px 100%, 0 calc(100% - 8px)); }
  .nav:hover { background: var(--ink-3); color: var(--paper); }
  .nav:focus-visible { outline: 2px solid var(--focus); outline-offset: -4px; border-radius: 0; }
  li.on .nav { color: var(--mango); background: var(--ink-3); }
  /* active indicator: a bar flush with the rail edge */
  li.on::before { content: ''; position: absolute; left: -14px; top: 8px; bottom: 8px; width: 3px; background: var(--accent-fill); z-index: 1; }
  .ring { display: block; width: 44px; height: 44px; padding: 2px; background: transparent; transition: background var(--dur) var(--ease); }
  .prof:hover .ring, .avatar:hover .ring { background: var(--line); }
  li.sel .ring { background: var(--accent-fill); }
  .prof:focus-visible, .avatar:focus-visible { outline: 2px solid var(--focus); outline-offset: 3px; border-radius: 2px; }
  .recent li .sq { position: absolute; right: -3px; bottom: -3px; width: 10px; height: 10px; border: 2px solid var(--ink-1); box-sizing: border-box; }
  .pulse { animation: pulse 1.6s ease-in-out infinite; }
  .cred { position: absolute; right: -3px; bottom: -3px; width: 12px; height: 12px; border: 2px solid var(--ink-1); box-sizing: border-box; }
  @media (max-height: 680px) { .recent li:nth-child(n + 4) { display: none; } }
  @keyframes pulse { 50% { opacity: 0.35; } }
</style>
