<script lang="ts">
  import ArrowsClockwiseIcon from 'phosphor-svelte/lib/ArrowsClockwiseIcon';
  import StarIcon from 'phosphor-svelte/lib/StarIcon';
  import UsersIcon from 'phosphor-svelte/lib/UsersIcon';
  import PlayIcon from 'phosphor-svelte/lib/PlayIcon';
  import SealCheckIcon from 'phosphor-svelte/lib/SealCheckIcon';
  import Tile from './Tile.svelte';
  import EmptyState from './EmptyState.svelte';
  import NoAccountHint from './NoAccountHint.svelte';
  import { stripMotd } from '../lib/format';
  import { actions, app, isBusy, nav, selectedProfile, servers, t } from '../lib/stores.svelte';
  import type { ServerEntry } from '../lib/types';

  let profileId = $state<string>('');
  const prof = $derived(app.profiles.find((p) => p.id === profileId) ?? selectedProfile());
  const sorted = $derived(
    [...servers.list].sort((a, b) => Number(b.partner) - Number(a.partner) || Number(b.starred) - Number(a.starred)),
  );
  $effect(() => { actions.loadServers(); });
  $effect(() => { if (!profileId && selectedProfile()) profileId = selectedProfile()?.id ?? ''; });

  function join(s: ServerEntry) {
    if (!prof) return;
    actions.selectProfile(prof.id);
    actions.play(prof.id, s.address);
    nav.view = 'home';
  }
</script>

<div class="sv">
  <header>
    <div><h1 class="display">{t('nav.servers')}</h1><div class="hint">{t('servers.hint')}</div></div>
    <div class="ctrl">
      <label class="f"><span class="hint">{t('servers.joinWith')}</span>
        <select class="field" bind:value={profileId}>{#each app.profiles as p (p.id)}<option value={p.id}>{p.name} ({p.mcVersion})</option>{/each}</select>
      </label>
      <button class="btn" type="button" disabled={servers.pinging} onclick={() => actions.pingServers()}>
        <span class:spin={servers.pinging} class="ic"><ArrowsClockwiseIcon weight="bold" size={16} aria-hidden="true" /></span>{t('servers.refresh')}
      </button>
    </div>
  </header>

  {#if !app.accounts.length}<NoAccountHint />{/if}

  {#if !servers.loaded}
    <ul class="grid">{#each Array(6) as _, i (i)}<li class="card sk"><span class="img"></span><span class="b"></span></li>{/each}</ul>
  {:else if sorted.length === 0}
    <EmptyState title={t('servers.emptyTitle')} hint={t('servers.empty')}>
      <button class="btn" type="button" onclick={() => actions.loadServers(true)}><ArrowsClockwiseIcon weight="bold" size={16} aria-hidden="true" />{t('servers.refresh')}</button>
    </EmptyState>
  {:else}
    <ul class="grid">
      {#each sorted as s (s.id)}
        {@const st = servers.status[s.id]}
        <li class="card" class:off={st && !st.online}>
          <Tile kind="server" id={s.id} src={s.iconUrl} size={64} radius={8} />
          <div class="info">
            <div class="l1">
              <b class="nm">{s.name}</b>
              {#if s.starred}<span class="star" title={t('servers.starred')}><StarIcon weight="fill" size={14} aria-hidden="true" /><span class="sr-only">{t('servers.starred')}</span></span>{/if}
              {#if s.partner}<span class="tag"><SealCheckIcon weight="bold" size={12} aria-hidden="true" />{t('servers.partner')}</span>{/if}
            </div>
            <div class="addr mono">{s.address}</div>
            <div class="motd">{#if !st}<span class="hint">{t('servers.pinging')}</span>{:else if st.online}{stripMotd(st.motd) || ' '}{:else}&nbsp;{/if}</div>
            <div class="stat mono">
              <span class="dot" class:on={st?.online} class:unk={!st} aria-hidden="true"></span>
              {#if st?.online}
                <span>{st.ping} ms</span>
                {#if st.players}<span><UsersIcon weight="bold" size={13} aria-hidden="true" /> {st.players.online}/{st.players.max}</span>{/if}
                {#if st.version}<span class="ver">{st.version}</span>{/if}
              {:else if st}<span>{t('servers.offline')}</span>{:else}<span>&hellip;</span>{/if}
            </div>
          </div>
          <button class="btn accent join" type="button" disabled={!prof || (prof ? isBusy(prof.id) : true) || !app.accounts.length} title={!app.accounts.length ? t('play.noAccount') : prof && isBusy(prof.id) ? t('servers.busy') : ''} onclick={() => join(s)}>
            <PlayIcon weight="bold" size={14} aria-hidden="true" />{t('servers.join')}
          </button>
        </li>
      {/each}
    </ul>
  {/if}
</div>

<style>
  .sv { height: 100%; overflow: auto; padding: var(--s-5) 32px 32px; display: flex; flex-direction: column; gap: var(--s-4); }
  header { display: flex; justify-content: space-between; align-items: flex-end; gap: 16px; flex-wrap: wrap; }
  h1 { font-size: var(--fs-3xl); }
  .ctrl { display: flex; gap: 10px; align-items: flex-end; }
  .f { display: grid; gap: 2px; font-size: 12px; }
  .grid { list-style: none; margin: 0; padding: 0; display: grid; grid-template-columns: repeat(auto-fill, minmax(430px, 1fr)); gap: 12px; }
  .card { display: grid; grid-template-columns: 64px minmax(0, 1fr) auto; gap: 14px; align-items: center; padding: 12px; background: var(--ink-2); border: 1px solid var(--line); border-radius: var(--r-m); }
  .card.off { opacity: 0.72; }
  .info { min-width: 0; display: grid; gap: 2px; }
  .l1 { display: flex; align-items: center; gap: 8px; min-width: 0; }
  .nm { font: 700 17px var(--font-display); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .star { color: var(--mango); display: inline-flex; }
  .tag { display: inline-flex; align-items: center; gap: 4px; font-size: 11.5px; padding: 1px 7px; border-radius: var(--r-s); background: rgba(255, 173, 50, 0.14); color: var(--mango); white-space: nowrap; }
  .addr { font-size: 12.5px; color: var(--paper-mute); }
  .motd { font-size: 13.5px; color: var(--paper-dim); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; min-height: 1.45em; }
  .stat { display: flex; flex-wrap: wrap; align-items: center; gap: 2px 12px; font-size: 12.5px; color: var(--paper-dim); }
  .stat span { display: inline-flex; align-items: center; gap: 4px; white-space: nowrap; }
  .ver { color: var(--paper-mute); }
  .dot { width: 8px; height: 8px; border-radius: 50%; background: var(--tomato); }
  .dot.on { background: var(--leaf); }
  .dot.unk { background: var(--paper-mute); }
  .ic { display: inline-flex; }
  .spin { animation: spin 1s linear infinite; }
  .sk .img { width: 64px; height: 64px; border-radius: 8px; background: var(--ink-3); animation: sh 1.4s ease-in-out infinite; }
  .sk .b { height: 14px; width: 200px; background: var(--ink-3); border-radius: 4px; animation: sh 1.4s ease-in-out infinite; }
  @keyframes sh { 50% { opacity: 0.5; } }
  @keyframes spin { to { transform: rotate(360deg); } }
</style>
