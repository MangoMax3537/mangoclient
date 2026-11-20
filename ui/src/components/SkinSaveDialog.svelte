<script lang="ts">
  // Saves the account's current skin into the library (`skin_library_save_current`) under a name.
  import { untrack } from 'svelte';
  import Modal from './Modal.svelte';
  import { errorText, getApi, t } from '../lib/stores.svelte';

  interface Props { uuid: string; defaultName: string; onclose: () => void; ondone: () => void }
  let { uuid, defaultName, onclose, ondone }: Props = $props();
  let name = $state(untrack(() => defaultName));
  let busy = $state(false);
  let error = $state('');
  async function save() {
    if (busy) return;
    busy = true; error = '';
    try { await getApi().skinLibrarySaveCurrent({ uuid, name: name.trim() || defaultName }); ondone(); }
    catch (e) { error = errorText(e); busy = false; }
  }
</script>

<Modal title={t('skins.save.title')} {onclose} onenter={save}>
  <div class="body">
    <p class="hint">{t('skins.save.hint')}</p>
    <label for="sk-save-name">{t('skins.name')}</label>
    <input id="sk-save-name" class="field" bind:value={name} maxlength="48" autocomplete="off" />
    {#if error}<div class="err" role="alert">{error}</div>{/if}
  </div>
  {#snippet footer()}
    <button class="btn ghost" type="button" onclick={onclose}>{t('common.cancel')}</button>
    <button class="btn accent" type="button" disabled={busy} onclick={save}>{t('skins.save.apply')}</button>
  {/snippet}
</Modal>

<style>
  .body { display: grid; gap: 8px; }
  p { margin: 0; }
  label { font-weight: 500; }
</style>
