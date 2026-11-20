<script lang="ts">
  import PlayIcon from 'phosphor-svelte/lib/PlayIcon';
  import ArrowRightIcon from 'phosphor-svelte/lib/ArrowRightIcon';
  import Tile from './Tile.svelte';
  import { actions, app, isBusy, nav, selectedProfile, servers, t } from '../lib/stores.svelte';

  const top = $derived(
    [...servers.list].sort((a, b) => Number(b.partner) - Number(a.partner) || Number(b.starred) - Number(a.starred)).slice(0, 4),
  );
  const prof = $derived(selectedProfile());
  $effect(() => { actions.loadServers(); });
</script>

<section class="qj" aria-labelledby="qj-h">
  <header>
    <h2 id="qj-h" class="eyebrow">{t('servers.quickJoin')}</h2>
    <button class="btn ghost" type="button" onclick={() => (nav.view = 'servers')}>{t('nav.servers')}<ArrowRightIcon weight="bold" size={14} aria-hidden="true" /></button>
  </header>
  {#if !servers.loaded}
    <ul>{#each Array(3) as _, i (i)}<li class="sk"><span class="img"></span><span class="b"></span></li>{/each}</ul>
  {:else if top.length === 0}
    <p class="hint">{t('servers.empty')}</p>
  {:else}
    <ul>
      {#each top as s (s.id)}
        {@const st = servers.status[s.id]}
        <li>
          <Tile kind="server" id={s.id} src={s.iconUrl} size={36} radius={6} />
          <div class="i">
            <b>{s.name}</b>
            <span class="m mono"><span class="dot" class:on={st?.online} class:unk={!st} aria-hidden="true"></span>{#if st?.online}{st.ping} ms{#if st.players}&nbsp;&middot;&nbsp;{st.players.online}{/if}{:else if st}{t('servers.offline')}{:else}&hellip;{/if}</span>
          </div>
          <button class="btn ghost ic" type="button" disabled={!prof || (prof ? isBusy(prof.id) : true) || !app.accounts.length} aria-label="{t('servers.join')}: {s.name}"
            onclick={() => { if (prof) { actions.play(prof.id, s.address); } }}><PlayIcon weight="bold" size={16} aria-hidden="true" /></button>
        </li>
      {/each}
    </ul>
  {/if}
</section>

<style>
  .qj { min-width: 0; }
  header { display: flex; justify-content: space-between; align-items: center; margin-bottom: 4px; }
  ul { list-style: none; margin: 0; padding: 0; }
  li { display: grid; grid-template-columns: 36px minmax(0, 1fr) 32px; gap: 10px; align-items: center; padding: 6px 4px; border-bottom: 1px solid var(--line-soft); }
  .i { display: grid; min-width: 0; line-height: 1.3; }
  .i b { font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .m { font-size: 12.5px; color: var(--paper-dim); display: inline-flex; align-items: center; gap: 6px; }
  .dot { width: 7px; height: 7px; border-radius: 50%; background: var(--tomato); }
  .dot.on { background: var(--leaf); }
  .dot.unk { background: var(--paper-mute); }
  .ic { width: 32px; height: 32px; padding: 0; justify-content: center; }
  .sk .img { width: 36px; height: 36px; border-radius: 6px; background: var(--ink-2); animation: sh 1.4s ease-in-out infinite; }
  .sk .b { height: 12px; width: 120px; background: var(--ink-2); border-radius: 4px; animation: sh 1.4s ease-in-out infinite; }
  @keyframes sh { 50% { opacity: 0.5; } }
</style>
