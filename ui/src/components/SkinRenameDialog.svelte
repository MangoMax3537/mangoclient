<script lang="ts">
  // Renames a saved library skin (`skin_library_rename`); the name is validated by the backend as well.
  import { untrack } from 'svelte';
  import Modal from './Modal.svelte';
  import { errorText, getApi, t } from '../lib/stores.svelte';
  import type { LibrarySkin } from '../lib/types';

  interface Props { skin: LibrarySkin; onclose: () => void; ondone: (s: LibrarySkin) => void }
  let { skin, onclose, ondone }: Props = $props();
  let name = $state(untrack(() => skin.name));
  let busy = $state(false);
  let error = $state('');
  async function save() {
    if (busy) return;
    const next = name.trim();
    if (!next) { error = errorText({ code: 'skin.invalid_name', message: '' }); return; }
    if (next === skin.name) { onclose(); return; }
    busy = true; error = '';
    try { ondone(await getApi().skinLibraryRename({ id: skin.id, name: next })); }
    catch (e) { error = errorText(e); busy = false; }
  }
</script>

<Modal title={t('skins.rename.title')} {onclose} onenter={save}>
  <div class="body">
    <label for="sk-ren-name">{t('skins.name')}</label>
    <input id="sk-ren-name" class="field" bind:value={name} maxlength="48" autocomplete="off" />
    {#if error}<div class="err" role="alert">{error}</div>{/if}
  </div>
  {#snippet footer()}
    <button class="btn ghost" type="button" onclick={onclose}>{t('common.cancel')}</button>
    <button class="btn accent" type="button" disabled={busy} onclick={save}>{t('skins.rename.apply')}</button>
  {/snippet}
</Modal>

<style>
  .body { display: grid; gap: 8px; }
  label { font-weight: 500; }
</style>
