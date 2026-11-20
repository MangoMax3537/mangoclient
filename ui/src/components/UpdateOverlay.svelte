<script lang="ts">
  import CircleNotchIcon from 'phosphor-svelte/lib/CircleNotchIcon';
  import ArrowCircleUpIcon from 'phosphor-svelte/lib/ArrowCircleUpIcon';
  import Modal from './Modal.svelte';
  import Brand from './Brand.svelte';
  import { actions, t, updates } from '../lib/stores.svelte';

  const u = $derived(updates.status);
</script>

<Modal title={updates.installing ? t('update.installing') : t('update.available', { v: u.version ?? '' })} locked={updates.installing} onclose={() => (updates.overlay = false)}>
  <div class="take" aria-live="polite">
    <Brand />
    {#if updates.installing}
      <span class="spin"><CircleNotchIcon weight="bold" size={32} aria-hidden="true" /></span>
      <p>{t('update.installingHint')}</p>
    {:else}
      <ArrowCircleUpIcon weight="bold" size={32} aria-hidden="true" />
      <p>{u.notes ?? t('update.noNotes')}</p>
      <p class="hint">{t('update.restartHint')}</p>
    {/if}
  </div>
  {#snippet footer()}
    {#if !updates.installing}
      <button class="btn ghost" type="button" onclick={() => (updates.overlay = false)}>{t('update.later')}</button>
      <button class="btn accent" type="button" onclick={() => actions.updateInstall()}>{t('update.install')}</button>
    {/if}
  {/snippet}
</Modal>

<style>
  .take { display: grid; gap: 10px; justify-items: start; color: var(--mango); }
  p { margin: 0; color: var(--paper); }
  .spin { display: inline-flex; animation: spin 1s linear infinite; }
  @keyframes spin { to { transform: rotate(360deg); } }
</style>
