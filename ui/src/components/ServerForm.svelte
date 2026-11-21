<script lang="ts">
  // Add or edit a server (name + address). Enter saves, errors from the backend (duplicate, invalid address) show inline. A server from the
  // game's own list keeps its address; only the launcher-side name can change.
  import { onMount, untrack } from 'svelte';
  import Modal from './Modal.svelte';
  import { actions, errorText, t } from '../lib/stores.svelte';
  import type { ServerEntry } from '../lib/types';

  interface Props { server?: ServerEntry | null; onclose: () => void }
  let { server = null, onclose }: Props = $props();
  let name = $state(untrack(() => server?.name ?? ''));
  let address = $state(untrack(() => server?.address ?? ''));
  let busy = $state(false);
  let err = $state('');
  let nameEl: HTMLInputElement | undefined = $state();
  const lockedAddress = $derived(!!server && !server.custom);
  const valid = $derived(name.trim().length > 0 && name.trim().length <= 64 && address.trim().length > 0 && !/\s/.test(address.trim()));

  onMount(() => { queueMicrotask(() => { nameEl?.focus(); nameEl?.select(); }); });
  async function save() {
    if (!valid || busy) return;
    busy = true; err = '';
    try {
      if (server) await actions.serverUpdate(server.id, name.trim(), lockedAddress ? undefined : address.trim());
      else await actions.serverAdd(name.trim(), address.trim());
      onclose();
    } catch (e) { err = errorText(e); busy = false; }
  }
</script>

<Modal title={server ? t('servers.editTitle') : t('servers.addTitle')} {onclose} onenter={() => void save()}>
  <div class="form">
    <label class="f"><span class="eyebrow">{t('servers.name')}</span>
      <input bind:this={nameEl} class="field" type="text" bind:value={name} maxlength="64" autocomplete="off" spellcheck="false" oninput={() => (err = '')} />
    </label>
    <label class="f"><span class="eyebrow">{t('servers.address')}</span>
      <input class="field mono" type="text" bind:value={address} maxlength="255" autocomplete="off" spellcheck="false" disabled={lockedAddress} aria-describedby="sf-hint" oninput={() => (err = '')} />
    </label>
    <p id="sf-hint" class="hint">{lockedAddress ? t('servers.addressLocked') : t('servers.addressHint')}</p>
    {#if err}<div class="err" role="alert">{err}</div>{/if}
  </div>
  {#snippet footer()}
    <button class="btn ghost" type="button" onclick={onclose}>{t('common.cancel')}</button>
    <button class="btn accent" type="button" disabled={!valid || busy} onclick={() => void save()}>{t('servers.save')}</button>
  {/snippet}
</Modal>

<style>
  .form { display: grid; gap: 12px; }
  .f { display: grid; gap: 6px; }
  .hint { margin: 0; }
</style>
