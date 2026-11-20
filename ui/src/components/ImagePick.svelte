<script lang="ts">
  // Drop zone + file button shared by the icon and background pickers. Dropping is wired by the parent (`use:imageDrop`).
  import UploadSimpleIcon from 'phosphor-svelte/lib/UploadSimpleIcon';
  import { pickImage, type PickedImage } from '../lib/pickImage';
  import { t } from '../lib/stores.svelte';

  let { onpick, hint, over = false, bad = false }: { onpick: (img: PickedImage) => void; hint: string; over?: boolean; bad?: boolean } = $props();
  async function choose() {
    const i = await pickImage();
    if (i) onpick(i);
  }
</script>

<div class="dz" class:over role="group" aria-label={t('icons.upload.zone')}>
  <span class="ic" aria-hidden="true"><UploadSimpleIcon weight="bold" size={28} /></span>
  <b>{t('icons.upload.drop')}</b>
  <button class="btn" type="button" onclick={choose}>{t('icons.upload.choose')}</button>
  <span class="hint">{hint}</span>
  {#if bad}<span class="err" role="alert">{t('icons.upload.badType')}</span>{/if}
</div>

<style>
  .dz { display: grid; justify-items: center; align-content: center; gap: 10px; min-height: 200px; padding: 24px; text-align: center; border: 2px dashed var(--line); border-radius: var(--r-m); background: var(--ink-1); transition: border-color var(--dur) var(--ease), background var(--dur) var(--ease); }
  .dz.over { border-color: var(--accent-fill); background: color-mix(in srgb, var(--ink-2) 88%, var(--accent-fill)); }
  .ic { color: var(--paper-dim); }
  .hint { max-width: 36ch; }
</style>
