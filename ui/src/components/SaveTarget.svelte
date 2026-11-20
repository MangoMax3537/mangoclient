<script lang="ts">
  // "Save as" row: the chosen path plus a button for the native save dialog (`pick_save_file`). Outside the Tauri shell there is no native
  // dialog, so the path can be typed (dev server and tests).
  import FolderOpenIcon from 'phosphor-svelte/lib/FolderOpenIcon';
  import { inTauri } from '../lib/api';
  import { t, getApi, errorText, toast } from '../lib/stores.svelte';
  import type { Key } from '../lib/i18n';

  interface Props { path: string; defaultName: string; title: string; label: string; placeholder?: string; disabled?: boolean; id?: string; invalidKey?: Key | null }
  let { path = $bindable(), defaultName, title, label, placeholder = '', disabled = false, id = 'save-target', invalidKey = null }: Props = $props();
  const typed = !inTauri();
  let choosing = $state(false);

  async function choose() {
    choosing = true;
    try {
      const p = await getApi().pickSaveFile({ title, defaultName });
      if (typeof p === 'string' && p) path = p;
    } catch (e) { toast(errorText(e), 'error'); }
    choosing = false;
  }
</script>

<div class="st">
  <label for={id}>{label}</label>
  <div class="row">
    <input {id} class="field mono" bind:value={path} readonly={!typed} {disabled} {placeholder} spellcheck="false" autocomplete="off" aria-invalid={!!invalidKey} aria-describedby={invalidKey ? `${id}-why` : undefined} />
    <button class="btn" type="button" disabled={disabled || choosing} onclick={choose}><FolderOpenIcon weight="bold" size={16} aria-hidden="true" />{t('export.choose')}</button>
  </div>
  {#if invalidKey}<div id="{id}-why" class="why" role="alert">{t(invalidKey)}</div>{/if}
</div>

<style>
  .st { display: grid; gap: 6px; }
  label { font-size: var(--fs-sm); font-weight: 600; color: var(--paper-dim); }
  .row { display: flex; gap: 8px; }
  .field { flex: 1; min-width: 0; }
  .why { font-size: var(--fs-sm); color: var(--tomato); }
</style>
