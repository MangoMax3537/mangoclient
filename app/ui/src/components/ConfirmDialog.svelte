<script lang="ts">
  import Modal from './Modal.svelte';
  import { t } from '../lib/stores.svelte';

  interface Props {
    title: string;
    body: string;
    confirmLabel: string;
    danger?: boolean;
    busy?: boolean;
    /** Information only: hides the confirm button. */
    infoOnly?: boolean;
    items?: string[];
    onconfirm: () => void;
    oncancel: () => void;
  }
  let { title, body, confirmLabel, danger = false, busy = false, infoOnly = false, items = [], onconfirm, oncancel }: Props = $props();
</script>

<Modal {title} onclose={oncancel}>
  <p class="msg">{body}</p>
  {#if items.length}
    <ul class="items">{#each items as it (it)}<li>{it}</li>{/each}</ul>
  {/if}
  {#snippet footer()}
    <button class="btn ghost" type="button" onclick={oncancel}>{infoOnly ? t('common.close') : t('common.cancel')}</button>
    {#if !infoOnly}
      <button class="btn" class:danger class:accent={!danger} type="button" disabled={busy} onclick={onconfirm}>{confirmLabel}</button>
    {/if}
  {/snippet}
</Modal>

<style>
  .msg { margin: 0 0 8px; color: var(--paper-dim); }
  .items { margin: 8px 0 0; padding: 0; list-style: none; display: flex; flex-wrap: wrap; gap: 6px; }
  .items li { padding: 3px 9px; border: 1px solid var(--line); border-radius: var(--r-s); background: var(--ink-3); font-size: 13px; }
</style>
