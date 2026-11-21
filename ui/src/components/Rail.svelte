<script lang="ts">
  import HouseIcon from 'phosphor-svelte/lib/HouseIcon';
  import BooksIcon from 'phosphor-svelte/lib/BooksIcon';
  import CompassIcon from 'phosphor-svelte/lib/CompassIcon';
  import HardDrivesIcon from 'phosphor-svelte/lib/HardDrivesIcon';
  import TShirtIcon from 'phosphor-svelte/lib/TShirtIcon';
  import ChartBarIcon from 'phosphor-svelte/lib/ChartBarIcon';
  import GearIcon from 'phosphor-svelte/lib/GearIcon';
  import PlusIcon from 'phosphor-svelte/lib/PlusIcon';
  import Tile from './Tile.svelte';
  import InstanceIcon from './InstanceIcon.svelte';
  import Accounts from './Accounts.svelte';
  import Skeleton from './skeleton/Skeleton.svelte';
  import { BusyGate } from '../lib/loadState.svelte';
  import { tip } from '../lib/tooltip';
  import { railTileHandlers } from '../lib/instanceCard';
  import { instanceMenu } from '../lib/instanceMenu';
  import { contextmenu } from '../lib/contextMenu.svelte';
  import { shortcutLabel, type ShortcutAction } from '../lib/keybinds';
  import { actions, app, isBusy, accountNeedsLogin, keystoreMissing, nav, openDiscover, openProfile, lang, platform, selectedAccount, selectedProfile, t } from '../lib/stores.svelte';

  const recent = $derived([...app.profiles].sort((a, b) => (b.lastPlayed ?? 0) - (a.lastPlayed ?? 0)).slice(0, 5));
  const acc = $derived(selectedAccount());
  const gate = new BusyGate();
  $effect(() => { gate.update(!app.ready); return () => gate.destroy(); });
  // credential-state dot on the account button: ok / not saved / sign-in needed (offline accounts have none)
  const cred = $derived(!acc || acc.type !== 'microsoft' ? null : accountNeedsLogin(acc) ? 'login' : (acc.credentialsLocked || acc.credentialsPersisted === false) ? 'warn' : 'ok');
  const credText = $derived(cred === 'login' ? t('rail.cred.login') : cred === 'warn' ? t('rail.cred.warn') : cred === 'ok' ? t('rail.cred.ok') : '');
  const items = $derived([
    { v: 'home', a: 'home', label: t('nav.home'), icon: HouseIcon, go: () => (nav.view = 'home') },
    { v: 'library', a: 'library', label: t('nav.library'), icon: BooksIcon, go: () => (nav.view = 'library') },
    { v: 'discover', a: 'discover', label: t('nav.discover'), icon: CompassIcon, go: () => openDiscover() },
    { v: 'servers', a: 'servers', label: t('nav.servers'), icon: HardDrivesIcon, go: () => (nav.view = 'servers') },
    { v: 'skins', a: 'skins', label: t('nav.skins'), icon: TShirtIcon, go: () => (nav.view = 'skins') },
    { v: 'stats', a: 'stats', label: t('nav.stats'), icon: ChartBarIcon, go: () => (nav.view = 'stats') },
  ] as { v: string; a: ShortcutAction; label: string; icon: typeof HouseIcon; go: () => void }[]);
  const ops = { select: (id: string) => void actions.selectProfile(id), open: (id: string) => openProfile(id), isSelected: (id: string) => selectedProfile()?.id === id };
  const withKey = (label: string, a: ShortcutAction): string => `${label} (${shortcutLabel(a, platform.mac, lang())})`;
</script>

<nav class="rail" aria-label={t('nav.main')}>
  <ul class="top">
    {#each items as it (it.v)}
      <li class:on={nav.view === it.v}>
        <button type="button" class="nav" use:tip={withKey(it.label, it.a)} aria-current={nav.view === it.v ? 'page' : undefined} aria-label={it.label} onclick={it.go}>
          <it.icon weight="bold" size={22} aria-hidden="true" />
        </button>
      </li>
    {/each}
  </ul>

  <ul class="recent" aria-label={t('nav.recent')} aria-busy={!app.ready}>
    {#if !app.ready && gate.show}
      {#each Array(4) as _, i (i)}<li aria-hidden="true"><span class="ring"><Skeleton variant="box" w={40} h={40} notch={7} /></span></li>{/each}
    {/if}
    {#each recent as p (p.id)}
      <!-- highlighted only where that instance is the subject: its own page, or Home while Home shows it. Never in Library, Discover, Servers, Stats, Settings. -->
      {@const sel = nav.view === 'profile' ? p.id === nav.profileId : nav.view === 'home' && p.id === selectedProfile()?.id}
      <li class:sel class:on={nav.view === 'profile' && p.id === nav.profileId} use:contextmenu={() => instanceMenu(p)}>
        <button type="button" class="prof" use:tip={p.name} aria-label={p.name} aria-current={sel ? 'true' : undefined} {...railTileHandlers(p.id, ops)}>
          <span class="ring notch" style:--n="9px"><InstanceIcon profile={p} size={40} notch={7} /></span>
        </button>
        {#if isBusy(p.id)}<span class="sq ok pulse" aria-hidden="true"></span><span class="sr-only">{t('home.running')}</span>{/if}
      </li>
    {/each}
  </ul>

  <ul class="bottom">
    <li>
      <button type="button" class="nav add" use:tip={withKey(t('rail.newInstance'), 'newProfile')} aria-label={t('rail.newInstance')} onclick={() => (nav.newProfile = true)}><PlusIcon weight="bold" size={22} aria-hidden="true" /></button>
    </li>
    <li class="acc">
      <button type="button" class="avatar" aria-haspopup="dialog" aria-expanded={nav.accounts} use:tip={`${acc?.name ?? t('nav.noAccount')}${credText ? ` · ${credText}` : ''}`}
        aria-label="{t('nav.accounts')}: {acc?.name ?? t('nav.noAccount')}{credText ? `. ${credText}` : ''}{keystoreMissing() ? `. ${t('persist.railHint')}` : ''}"
        onclick={(e) => { e.stopPropagation(); nav.accounts = !nav.accounts; }}>
        <span class="ring">{#if app.ready}<Tile kind="account" id={acc?.uuid ?? ''} src={acc?.headUrl} size={40} />{:else if gate.show}<Skeleton variant="box" w={40} h={40} />{:else}<span class="ph"></span>{/if}</span>
        {#if cred}<span class="sq cred" class:ok={cred === 'ok'} class:warn={cred === 'warn'} class:bad={cred === 'login'} aria-hidden="true"></span>{/if}
      </button>
      {#if nav.accounts}<Accounts />{/if}
    </li>
    <li class:on={nav.view === 'settings'}>
      <button type="button" class="nav" use:tip={withKey(t('nav.settings'), 'settings')} aria-current={nav.view === 'settings' ? 'page' : undefined} aria-label={t('nav.settings')} onclick={() => (nav.view = 'settings')}><GearIcon weight="bold" size={22} aria-hidden="true" /></button>
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
  .nav:focus-visible { outline: 2px solid var(--focus); outline-offset: -4px; }
  li.on .nav { color: var(--mango); background: var(--ink-3); }
  /* active indicator: a bar flush with the rail edge */
  li.on::before { content: ''; position: absolute; left: -14px; top: 8px; bottom: 8px; width: 3px; background: var(--accent-fill); z-index: 1; }
  .ring { display: block; width: 44px; height: 44px; padding: 2px; background: transparent; transition: background var(--dur) var(--ease); }
  li.sel .ring { background: var(--accent-fill); }
  .prof:focus-visible, .avatar:focus-visible { outline: 2px solid var(--focus); outline-offset: 3px; }
  .recent li .sq { position: absolute; right: -3px; bottom: -3px; width: 10px; height: 10px; border: 2px solid var(--ink-1); box-sizing: border-box; }
  .ph { display: block; width: 40px; height: 40px; }
  .pulse { animation: pulse 1.6s ease-in-out infinite; }
  .cred { position: absolute; right: -3px; bottom: -3px; width: 12px; height: 12px; border: 2px solid var(--ink-1); box-sizing: border-box; }
  @media (max-height: 680px) { .recent li:nth-child(n + 4) { display: none; } }
  @keyframes pulse { 50% { opacity: 0.35; } }
</style>
