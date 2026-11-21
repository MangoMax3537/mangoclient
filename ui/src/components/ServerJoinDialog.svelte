<script lang="ts">
  // "Choose an instance" for joining a server (Servers view, Home quick join). Same dark angular dialog as the install target picker.
  // Fitting instances come first (the selected one on top), "not sure" ones are allowed, instances that do not fit or already run are disabled
  // with the reason. Picking a row launches that instance and quick-joins the server.
  import { onMount } from 'svelte';
  import CheckIcon from 'phosphor-svelte/lib/CheckIcon';
  import ProhibitIcon from 'phosphor-svelte/lib/ProhibitIcon';
  import QuestionIcon from 'phosphor-svelte/lib/QuestionIcon';
  import Modal from './Modal.svelte';
  import Tile from './Tile.svelte';
  import InstanceIcon from './InstanceIcon.svelte';
  import InfoLine from './InfoLine.svelte';
  import { joinRows } from '../lib/serverList';
  import { rangeLabel } from '../lib/serverVersion';
  import { versionLoaderFacts } from '../lib/infoLine';
  import { app, isBusy, launchJoin, selectedProfile, servers, t } from '../lib/stores.svelte';
  import type { ServerEntry } from '../lib/types';

  interface Props { server: ServerEntry; onclose: () => void }
  let { server, onclose }: Props = $props();

  const st = $derived(servers.status[server.id]);
  const range = $derived(rangeLabel(st?.versionRange));
  const rows = $derived(joinRows(app.profiles, st, selectedProfile()?.id ?? null, isBusy));
  const profileOf = (id: string) => app.profiles.find((p) => p.id === id);
  let listEl: HTMLUListElement | undefined = $state();

  onMount(() => { queueMicrotask(() => listEl?.querySelector<HTMLButtonElement>('button:not([disabled])')?.focus()); });
  const reason = (r: { reason: 'incompatible' | 'busy' | null }) =>
    r.reason === 'busy' ? t('servers.reason.busy') : t('servers.reason.incompatible', { range: range ?? '?' });
</script>

<Modal title={t('servers.joinTitle', { name: server.name })} {onclose} size="lg">
  <div class="head">
    <Tile kind="server" id={server.id} src={st?.iconUrl ?? server.iconUrl} size={40} radius={8} />
    <div class="ht">
      <p class="hint">{t('servers.joinHint')}</p>
      <p class="hint mono sup">{range ? t('servers.supports', { range }) : t('servers.versionUnknown')}</p>
    </div>
  </div>

  {#if rows.length === 0}
    <p class="hint">{t('servers.none')}</p>
  {:else}
    <ul class="list" aria-label={t('servers.joinList')} bind:this={listEl}>
      {#each rows as r (r.id)}
        {@const p = profileOf(r.id)}
        {#if p}
          <li>
            <button type="button" class="row" class:dark={!r.enabled} disabled={!r.enabled} onclick={() => launchJoin(p.id, server)}>
              <InstanceIcon profile={p} size={44} notch={8} />
              <span class="who">
                <b class="nm">{p.name}</b>
                <span class="mono facts2"><InfoLine facts={versionLoaderFacts(p, t)} /></span>
              </span>
              <span class="state" class:why={!r.enabled} class:ok={r.compat === 'yes'}>
                {#if !r.enabled}<ProhibitIcon weight="bold" size={14} aria-hidden="true" /><span>{reason(r)}</span>
                {:else if r.compat === 'yes'}<CheckIcon weight="bold" size={14} aria-hidden="true" /><span>{t('servers.fits')}</span>
                {:else}<QuestionIcon weight="bold" size={14} aria-hidden="true" /><span>{t('servers.notSure')}</span>{/if}
              </span>
            </button>
          </li>
        {/if}
      {/each}
    </ul>
  {/if}

  {#snippet footer()}
    <button class="btn ghost" type="button" onclick={onclose}>{t('common.cancel')}</button>
  {/snippet}
</Modal>

<style>
  .head { display: flex; align-items: center; gap: 12px; margin-bottom: 12px; }
  .ht { display: grid; gap: 2px; min-width: 0; }
  .head p { margin: 0; }
  .sup { font-size: var(--fs-sm); }
  .list { list-style: none; margin: 0; padding: 0; display: grid; border: 1px solid var(--line); overflow: hidden; }
  .list li { border-bottom: 1px solid var(--line-soft); }
  .list li:last-child { border-bottom: 0; }
  .row { display: grid; grid-template-columns: 44px minmax(0, 1fr) auto; gap: 12px; align-items: center; width: 100%; padding: 8px 12px; text-align: left; background: var(--ink-2); transition: background var(--dur) var(--ease); }
  .row:hover:not(:disabled) { background: var(--ink-3); }
  .row:focus-visible { outline: 2px solid var(--focus); outline-offset: -2px; }
  .row.dark { background: var(--ink-1); cursor: not-allowed; }
  .row.dark :global(.tile), .row.dark :global(canvas) { opacity: 0.4; filter: grayscale(1); }
  .row.dark .who { opacity: 0.6; }
  .who { display: grid; gap: 1px; min-width: 0; }
  .nm { font: 700 var(--fs-lg) var(--font-display); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .facts2 { font-size: var(--fs-sm); color: var(--paper-dim); }
  .state { display: inline-flex; align-items: center; gap: 6px; max-width: 250px; font-size: var(--fs-sm); color: var(--paper-dim); text-align: right; }
  .state.ok { color: var(--leaf); }
</style>
