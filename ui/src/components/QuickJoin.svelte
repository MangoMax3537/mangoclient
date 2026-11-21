<script lang="ts">
  // Home "Quick join": the same cards, size and strip as the instances above. Card = the server's real favicon, name, supported versions,
  // players and ping (facts only). Click: the selected instance starts and joins when it fits, otherwise the instance chooser opens.
  import ArrowRightIcon from 'phosphor-svelte/lib/ArrowRightIcon';
  import MediaCard from './MediaCard.svelte';
  import MediaStrip from './MediaStrip.svelte';
  import InfoLine from './InfoLine.svelte';
  import Tile from './Tile.svelte';
  import Skeleton from './skeleton/Skeleton.svelte';
  import JumpBackSkeleton from './skeleton/JumpBackSkeleton.svelte';
  import { BusyGate } from '../lib/loadState.svelte';
  import { arrangeServers, serverFacts } from '../lib/serverList';
  import { contextmenu } from '../lib/contextMenu.svelte';
  import { serverMenu } from '../lib/menus';
  import { actions, app, requestJoin, servers, t } from '../lib/stores.svelte';
  import { nav } from '../lib/stores.svelte';

  const top = $derived(arrangeServers(servers.list, servers.status, 'manual').slice(0, 10));
  $effect(() => { actions.loadServers(); });
  const gate = new BusyGate();
  $effect(() => { gate.update(!servers.loaded); return () => gate.destroy(); });
</script>

{#if !servers.loaded || top.length > 0}
<section class="qj" aria-labelledby="qj-h">
  <header>
    <h2 id="qj-h">{t('servers.quickJoin')}</h2>
    <button class="btn text" type="button" onclick={() => (nav.view = 'servers')}>{t('nav.servers')}<ArrowRightIcon weight="bold" size={14} aria-hidden="true" /></button>
  </header>
  {#if !servers.loaded}
    {#if gate.show}<JumpBackSkeleton count={5} />{/if}
  {:else}
    <MediaStrip label={t('servers.quickJoin')}>
      {#each top as s (s.id)}
        {@const st = servers.status[s.id]}
        {@const f = serverFacts(st, t)}
        <li use:contextmenu={() => serverMenu(s, { join: requestJoin, ping: () => actions.pingServers(), canJoin: app.accounts.length > 0 })}>
          <MediaCard title={s.name} playBadge dimmed={!!st && !st.online} onclick={() => requestJoin(s)} ariaLabel="{t('servers.join')}: {s.name}">
            {#snippet art()}
              {#if !(st?.iconUrl ?? s.iconUrl) && !st}<Skeleton variant="box" w="100%" h="100%" radius={0} />{:else}<span class="fav"><Tile kind="server" id={s.id} src={st?.iconUrl ?? s.iconUrl} size={96} radius={0} /></span>{/if}
            {/snippet}
            {#snippet lines()}
              <InfoLine facts={f.version} />
              {#if st}<InfoLine facts={f.stats} />{:else}<Skeleton variant="bar" lh={20} h={11} w="55%" />{/if}
            {/snippet}
          </MediaCard>
        </li>
      {/each}
    </MediaStrip>
  {/if}
</section>
{/if}

<style>
  .qj { min-width: 0; }
  header { display: flex; justify-content: space-between; align-items: baseline; margin-bottom: var(--s-3); }
  h2 { font-size: var(--fs-xl); }
  /* a favicon is a small square: shown whole and pixel-sharp on a neutral surface, never cropped into the 16:10 area */
  .fav { display: grid; place-items: center; width: 100%; height: 100%; background: var(--ink-3); }
  .fav :global(img) { image-rendering: pixelated; }
</style>
