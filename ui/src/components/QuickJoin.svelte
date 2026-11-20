<script lang="ts">
  import PlayIcon from 'phosphor-svelte/lib/PlayIcon';
  import ArrowRightIcon from 'phosphor-svelte/lib/ArrowRightIcon';
  import Tile from './Tile.svelte';
  import ListRowSkeleton from './skeleton/ListRowSkeleton.svelte';
  import { BusyGate } from '../lib/loadState.svelte';
  import { tip } from '../lib/tooltip';
  import { actions, app, isBusy, nav, selectedProfile, servers, t } from '../lib/stores.svelte';

  const top = $derived(
    [...servers.list].sort((a, b) => Number(b.partner) - Number(a.partner) || Number(b.starred) - Number(a.starred)).slice(0, 4),
  );
  const prof = $derived(selectedProfile());
  $effect(() => { actions.loadServers(); });
  const gate = new BusyGate();
  $effect(() => { gate.update(!servers.loaded); return () => gate.destroy(); });
</script>

<section class="qj" aria-labelledby="qj-h">
  <header>
    <h2 id="qj-h">{t('servers.quickJoin')}</h2>
    <button class="btn text" type="button" onclick={() => (nav.view = 'servers')}>{t('nav.servers')}<ArrowRightIcon weight="bold" size={14} aria-hidden="true" /></button>
  </header>
  {#if !servers.loaded}
    {#if gate.show}<ListRowSkeleton count={4} img={32} rowHeight={50} padding="0" gap={12} bars={[{ w: '36%', h: 13 }, { w: '22%', h: 10 }]} tail={32} />{/if}
  {:else if top.length === 0}
    <p class="hint">{t('servers.empty')}</p>
  {:else}
    <ul>
      {#each top as s (s.id)}
        {@const st = servers.status[s.id]}
        {@const disabled = !prof || isBusy(prof.id) || !app.accounts.length}
        <li>
          <Tile kind="server" id={s.id} src={s.iconUrl} size={32} notch={6} />
          <div class="i">
            <b>{s.name}</b>
            <span class="m"><span class="sq" class:ok={st?.online} class:bad={st && !st.online} aria-hidden="true"></span>
              <span class="mono">{#if st?.online}<span class="ping">{st.ping}</span> ms{#if st.players}<span class="sep">/</span>{st.players.online}{/if}{:else if st}{t('servers.offline')}{:else}&hellip;{/if}</span></span>
          </div>
          <button class="btn ghost ic" type="button" {disabled} use:tip={{ text: `${t('servers.join')}: ${s.name}`, side: 'top' }} aria-label="{t('servers.join')}: {s.name}"
            onclick={() => { if (prof) { actions.play(prof.id, s.address); } }}><PlayIcon weight="bold" size={16} aria-hidden="true" /></button>
        </li>
      {/each}
    </ul>
  {/if}
</section>

<style>
  .qj { min-width: 0; }
  header { display: flex; justify-content: space-between; align-items: baseline; margin-bottom: var(--s-2); }
  h2 { font-size: var(--fs-xl); }
  ul { list-style: none; margin: 0; padding: 0; }
  li { display: grid; grid-template-columns: 32px minmax(0, 1fr) 32px; gap: 12px; align-items: center; padding: 7px 0; border-top: 1px solid var(--line-soft); }
  li:last-child { border-bottom: 1px solid var(--line-soft); }
  .i { display: grid; min-width: 0; line-height: 1.3; }
  .i b { font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .m { font-size: var(--fs-sm); color: var(--paper-dim); display: inline-flex; align-items: center; gap: 8px; }
  .ping { color: var(--paper); font-weight: 500; }
  .sep { margin: 0 6px; opacity: 0.45; }
  .ic { width: 32px; height: 32px; padding: 0; }
</style>
