<script lang="ts">
  // Servers: one angular row per server (favicon, name, address, players, ping, star, Partner badge). Add, edit, remove, star, sort and search live
  // here; Join always opens the instance chooser unless the selected instance is known to fit (see requestJoin). The partner server is pinned first.
  import ArrowsClockwiseIcon from 'phosphor-svelte/lib/ArrowsClockwiseIcon';
  import StarIcon from 'phosphor-svelte/lib/StarIcon';
  import PlusIcon from 'phosphor-svelte/lib/PlusIcon';
  import PlayIcon from 'phosphor-svelte/lib/PlayIcon';
  import PencilSimpleIcon from 'phosphor-svelte/lib/PencilSimpleIcon';
  import TrashIcon from 'phosphor-svelte/lib/TrashIcon';
  import MagnifyingGlassIcon from 'phosphor-svelte/lib/MagnifyingGlassIcon';
  import SealCheckIcon from 'phosphor-svelte/lib/SealCheckIcon';
  import Tile from './Tile.svelte';
  import InfoLine from './InfoLine.svelte';
  import Skeleton from './skeleton/Skeleton.svelte';
  import ServerCardSkeleton from './skeleton/ServerCardSkeleton.svelte';
  import ServerForm from './ServerForm.svelte';
  import ConfirmDialog from './ConfirmDialog.svelte';
  import { BusyGate, LOAD_TIMEOUT_MS } from '../lib/loadState.svelte';
  import LoadError from './LoadError.svelte';
  import EmptyState from './EmptyState.svelte';
  import NoAccountHint from './NoAccountHint.svelte';
  import { tip } from '../lib/tooltip';
  import { contextmenu } from '../lib/contextMenu.svelte';
  import { serverMenu } from '../lib/menus';
  import { arrangeServers, SERVER_SORTS, serverFacts, type ServerSort } from '../lib/serverList';
  import { actions, app, applyPinged, errorText, getApi, requestJoin, servers, setServerSort, t, toast } from '../lib/stores.svelte';
  import type { Key } from '../lib/i18n';
  import type { ServerEntry } from '../lib/types';

  let query = $state('');
  const shown = $derived(arrangeServers(servers.list, servers.status, servers.sort, query));
  $effect(() => { actions.loadServers(); });
  const gate = new BusyGate();
  $effect(() => { gate.update(!servers.loaded); return () => gate.destroy(); });
  // statuses arrive one by one: the row shell (icon, name, address) stays, only the status area is a skeleton while the ping is pending
  const pingGate = new BusyGate();
  const pendingAny = $derived(servers.loaded && shown.some((s) => !servers.status[s.id]));
  let pingLate = $state(false);
  $effect(() => {
    pingGate.update(pendingAny);
    pingLate = false;
    if (!pendingAny) return () => pingGate.destroy();
    const h = setTimeout(() => { pingLate = true; }, LOAD_TIMEOUT_MS); // a ping that never answers ends in a message with Retry, not an endless skeleton
    return () => { clearTimeout(h); pingGate.destroy(); };
  });

  let form = $state<{ server: ServerEntry | null } | null>(null);
  let removing = $state<ServerEntry | null>(null);
  let removeBusy = $state(false);

  async function pingOne(s: ServerEntry) {
    try { applyPinged(await getApi().serversPing({ address: s.address })); } catch (e) { toast(errorText(e), 'error'); }
  }
  async function star(s: ServerEntry) {
    try { await actions.serverStar(s.id, !s.starred); } catch (e) { toast(errorText(e), 'error'); }
  }
  async function confirmRemove() {
    const s = removing;
    if (!s) return;
    removeBusy = true;
    try { await actions.serverRemove(s.id); removing = null; } catch (e) { toast(errorText(e), 'error'); }
    removeBusy = false;
  }
  const manage = { edit: (s: ServerEntry) => (form = { server: s }), star: (s: ServerEntry) => void star(s), remove: (s: ServerEntry) => (removing = s) };
</script>

<div class="sv">
  <header>
    <div><h1 class="display">{t('nav.servers')}</h1><div class="hint">{t('servers.hint')}</div></div>
    <div class="ctrl">
      <button class="btn" type="button" disabled={servers.pinging} onclick={() => actions.pingServers()}>
        <span class:spin={servers.pinging} class="ic"><ArrowsClockwiseIcon weight="bold" size={16} aria-hidden="true" /></span>{t('servers.refresh')}
      </button>
      <button class="btn accent" type="button" onclick={() => (form = { server: null })}><PlusIcon weight="bold" size={16} aria-hidden="true" />{t('servers.add')}</button>
    </div>
  </header>

  <div class="bar">
    <label class="search"><MagnifyingGlassIcon weight="bold" size={16} aria-hidden="true" />
      <input class="field" type="search" bind:value={query} placeholder={t('servers.search')} aria-label={t('servers.search')} autocomplete="off" spellcheck="false" />
    </label>
    <label class="sort"><span class="hint">{t('servers.sort')}</span>
      <select class="field" value={servers.sort} onchange={(e) => setServerSort(e.currentTarget.value as ServerSort)}>
        {#each SERVER_SORTS as k (k)}<option value={k}>{t(`servers.sort.${k}` as Key)}</option>{/each}
      </select>
    </label>
  </div>

  {#if !app.accounts.length}<NoAccountHint />{/if}
  {#if pingLate}<LoadError compact error={{ code: 'net.timeout', message: '', needsRelogin: false }} onretry={() => actions.pingServers()} />{/if}

  {#if !servers.loaded}
    {#if gate.show}<ServerCardSkeleton count={6} />{/if}
  {:else if servers.list.length === 0}
    <EmptyState title={t('servers.emptyTitle')} hint={t('servers.empty')}>
      <button class="btn accent" type="button" onclick={() => (form = { server: null })}><PlusIcon weight="bold" size={16} aria-hidden="true" />{t('servers.add')}</button>
    </EmptyState>
  {:else if shown.length === 0}
    <p class="hint">{t('servers.noMatch')}</p>
  {:else}
    <ul class="list">
      {#each shown as s (s.id)}
        {@const st = servers.status[s.id]}
        {@const icon = st?.iconUrl ?? s.iconUrl}
        {@const f = serverFacts(st, t)}
        <li class="row nb" style:--n="12px" class:off={st && !st.online} use:contextmenu={() => serverMenu(s, { join: (x) => requestJoin(x, { chooser: true }), ping: pingOne, canJoin: app.accounts.length > 0, manage })}>
          <!-- icon: the stored favicon at once, the one a ping returns as soon as it arrives; the fallback only when there is no icon at all -->
          {#if !icon && !st && !pingLate}
            <span class="slot">{#if pingGate.show}<Skeleton variant="box" w={56} h={56} radius={6} />{/if}</span>
          {:else}
            <Tile kind="server" id={s.id} src={icon} size={56} radius={6} />
          {/if}
          <div class="info">
            <div class="l1">
              <b class="nm">{s.name}</b>
              {#if s.partner}<span class="tag"><SealCheckIcon weight="bold" size={12} aria-hidden="true" />{t('servers.partner')}</span>{/if}
            </div>
            <div class="addr mono">{s.address}</div>
          </div>
          <div class="stats mono">
            {#if !st && pingLate}<span class="hint">{t('load.timeout')}</span>
            {:else if !st && pingGate.show}<Skeleton variant="bar" lh={20} h={12} w={150} />
            {:else if st}<InfoLine facts={f.stats} />{/if}
          </div>
          <div class="acts">
            {#if s.partner}
              <span class="star pinned" use:tip={{ text: t('servers.pinned'), side: 'top' }} role="img" aria-label={t('servers.pinned')}><StarIcon weight="fill" size={16} aria-hidden="true" /></span>
            {:else}
              <button class="ib" class:on={s.starred} type="button" aria-pressed={s.starred} aria-label={s.starred ? t('servers.unstar') : t('servers.star')} use:tip={{ text: s.starred ? t('servers.unstar') : t('servers.star'), side: 'top' }} onclick={() => star(s)}>
                <StarIcon weight={s.starred ? 'fill' : 'bold'} size={16} aria-hidden="true" />
              </button>
              <button class="ib hov" type="button" aria-label={t('servers.edit')} use:tip={{ text: t('servers.edit'), side: 'top' }} onclick={() => (form = { server: s })}><PencilSimpleIcon weight="bold" size={16} aria-hidden="true" /></button>
              <button class="ib hov" type="button" aria-label={t('servers.remove')} use:tip={{ text: t('servers.remove'), side: 'top' }} onclick={() => (removing = s)}><TrashIcon weight="bold" size={16} aria-hidden="true" /></button>
            {/if}
          </div>
          <button class="btn accent join" type="button" disabled={!app.accounts.length} title={!app.accounts.length ? t('play.noAccount') : ''} onclick={() => requestJoin(s, { chooser: true })}>
            <PlayIcon weight="bold" size={14} aria-hidden="true" />{t('servers.join')}
          </button>
        </li>
      {/each}
    </ul>
  {/if}
</div>

{#if form}<ServerForm server={form.server} onclose={() => (form = null)} />{/if}
{#if removing}
  <ConfirmDialog danger busy={removeBusy} title={t('servers.removeTitle', { name: removing.name })} body={t('servers.removeBody')} confirmLabel={t('servers.remove')} onconfirm={confirmRemove} oncancel={() => (removing = null)} />
{/if}

<style>
  .sv { height: 100%; overflow: auto; padding: var(--s-5) 32px 32px; display: flex; flex-direction: column; gap: var(--s-4); }
  header { display: flex; justify-content: space-between; align-items: flex-end; gap: 16px; flex-wrap: wrap; }
  h1 { font-size: var(--fs-3xl); }
  .ctrl { display: flex; gap: 10px; align-items: flex-end; }
  .bar { display: flex; gap: var(--s-3); align-items: flex-end; flex-wrap: wrap; }
  .search { position: relative; flex: 1; min-width: 220px; display: block; }
  .search :global(svg) { position: absolute; left: 12px; top: 10px; color: var(--paper-mute); pointer-events: none; }
  .search .field { width: 100%; padding-left: 36px; }
  .sort { display: grid; gap: 4px; font-size: 12px; line-height: 1.3; min-width: 12rem; }
  .sort .field { width: 100%; font-size: var(--fs-md); line-height: normal; text-overflow: ellipsis; }
  .list { list-style: none; margin: 0; padding: 0; display: grid; gap: 10px; }
  .row { display: grid; grid-template-columns: 56px minmax(0, 1fr) max-content auto auto; gap: 16px; align-items: center; padding: 12px 14px; }
  .row.off { opacity: 0.72; }
  .info { min-width: 0; display: grid; gap: 2px; }
  .l1 { display: flex; align-items: center; gap: 8px; min-width: 0; }
  .nm { font: 700 17px/1.25 var(--font-display); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .tag { display: inline-flex; align-items: center; gap: 4px; font-size: 11.5px; padding: 1px 7px; background: rgba(255, 173, 50, 0.14); color: var(--mango); white-space: nowrap; }
  .addr { font-size: 12.5px; color: var(--paper-mute); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  /* players and ping are never squeezed into each other: the cell is as wide as its facts, nothing is clipped or overlapped */
  .stats { font-size: 12.5px; color: var(--paper-dim); min-width: 0; padding-inline: 2px; }
  .stats :global(.il) { overflow: visible; }
  .stats :global(.it) { flex: none; }
  .acts { display: flex; align-items: center; gap: 2px; }
  .ib, .star { width: 32px; height: 32px; display: grid; place-items: center; color: var(--paper-mute); }
  .ib:hover { color: var(--paper); background: var(--ink-3); }
  .ib.on, .star.pinned { color: var(--mango); }
  .ib.hov { opacity: 0; transition: opacity var(--dur) var(--ease); }
  .row:hover .ib.hov, .row:focus-within .ib.hov { opacity: 1; }
  .ic { display: inline-flex; }
  .slot { display: block; width: 56px; height: 56px; }
  .spin { animation: spin 1s linear infinite; }
  @keyframes spin { to { transform: rotate(360deg); } }
  @media (max-width: 1100px) { .row { grid-template-columns: 56px minmax(0, 1fr) auto auto; } .stats { display: none; } }
</style>
