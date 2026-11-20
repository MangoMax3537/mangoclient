<script lang="ts">
  // Upload a skin file to the account (`skin_upload`) or add it to the library (`skin_library_add`). The file comes from the native dialog
  // (`pick_image_file`); the model (classic / slim) is chosen here. Where the browser gives a blob URL the chosen file is previewed before
  // anything is applied; in the packaged app the webview cannot read the picked path, so the file name stands in for the preview.
  import FolderOpenIcon from 'phosphor-svelte/lib/FolderOpenIcon';
  import Modal from './Modal.svelte';
  import Seg from './Seg.svelte';
  import SkinFlat from './SkinFlat.svelte';
  import { pickImage, type PickedImage } from '../lib/pickImage';
  import { nameFromFile } from '../lib/skinFlows';
  import { errorText, getApi, t } from '../lib/stores.svelte';
  import { toAppError } from '../lib/api';
  import type { SkinInfo, SkinVariantName } from '../lib/types';

  interface Props {
    mode: 'upload' | 'library';
    uuid: string;
    onclose: () => void;
    /** `upload`: the new skin info. `library`: undefined (the caller reloads the library). */
    ondone: (info?: SkinInfo) => void;
    /** Called for `auth.needs_relogin` so the page can open the relogin dialog and retry. */
    onrelogin?: (retry: () => void) => void;
  }
  let { mode, uuid, onclose, ondone, onrelogin }: Props = $props();

  let picked = $state<PickedImage | null>(null);
  let variant = $state<SkinVariantName>('classic');
  let name = $state('');
  let busy = $state(false);
  let error = $state('');

  async function choose() {
    error = '';
    try {
      const p = await pickImage(t('skins.pick.title'));
      if (p) { picked = p; if (!name) name = nameFromFile(p.name); }
    } catch (e) { error = errorText(e); }
  }
  async function apply() {
    if (!picked || busy) return;
    busy = true; error = '';
    try {
      if (mode === 'upload') ondone(await getApi().skinUpload({ uuid, filePath: picked.path, variant }));
      else { await getApi().skinLibraryAdd({ filePath: picked.path, name: name.trim() || nameFromFile(picked.name), variant }); ondone(); }
    } catch (e) {
      if (toAppError(e).needsRelogin && onrelogin) { onrelogin(() => void apply()); onclose(); return; }
      error = errorText(e);
    }
    busy = false;
  }
</script>

<Modal title={mode === 'upload' ? t('skins.upload.title') : t('skins.addLib.title')} {onclose} onenter={apply}>
  <div class="body">
    <p class="hint">{mode === 'upload' ? t('skins.upload.hint') : t('skins.addLib.hint')}</p>
    <div class="file">
      <button class="btn" type="button" onclick={choose}><FolderOpenIcon weight="bold" size={16} aria-hidden="true" />{picked ? t('skins.pick.another') : t('skins.pick.choose')}</button>
      <span class="fname mono">{picked ? picked.name : t('skins.pick.none')}</span>
    </div>

    <div class="row">
      <div class="prev" aria-live="polite">
        {#if picked?.preview}
          <div class="fig"><SkinFlat url={picked.preview} slim={variant === 'slim'} side="front" scale={6} label={t('skins.preview.front')} /></div>
          <div class="fig"><SkinFlat url={picked.preview} slim={variant === 'slim'} side="back" scale={6} label={t('skins.preview.back')} /></div>
        {:else if picked}
          <p class="hint nopv">{t('skins.preview.unavailable')}</p>
        {/if}
      </div>
      <div class="opts">
        <span class="lbl">{t('skins.model')}</span>
        <Seg label={t('skins.model')} value={variant} idPrefix="skmodel" options={[{ value: 'classic', label: t('skins.classic') }, { value: 'slim', label: t('skins.slim') }]} onchange={(v) => (variant = v as SkinVariantName)} />
        <span class="hint">{variant === 'slim' ? t('skins.slim.hint') : t('skins.classic.hint')}</span>
        {#if mode === 'library'}
          <label class="nm" for="sk-name"><span class="lbl">{t('skins.name')}</span></label>
          <input id="sk-name" class="field" bind:value={name} maxlength="48" autocomplete="off" />
        {/if}
      </div>
    </div>
    {#if error}<div class="err" role="alert">{error}</div>{/if}
  </div>
  {#snippet footer()}
    <button class="btn ghost" type="button" onclick={onclose}>{t('common.cancel')}</button>
    <button class="btn accent" type="button" disabled={!picked || busy} aria-busy={busy} onclick={apply}>{mode === 'upload' ? t('skins.upload.apply') : t('skins.addLib.apply')}</button>
  {/snippet}
</Modal>

<style>
  .body { display: grid; gap: 14px; }
  p { margin: 0; }
  .file { display: flex; align-items: center; gap: 12px; min-width: 0; }
  .fname { color: var(--paper-dim); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: var(--fs-sm); }
  .row { display: flex; gap: 20px; align-items: flex-start; }
  .prev { display: flex; gap: 10px; flex: none; }
  .prev:empty { display: none; }
  .fig { width: 72px; height: 144px; background: var(--ink-3); padding: 4px; }
  .opts { display: grid; gap: 8px; justify-items: start; min-width: 0; flex: 1; }
  .opts .field { width: 100%; }
  .lbl { font-weight: 500; }
  .nm { margin-top: 6px; }
  .nopv { max-width: 28ch; }
</style>
