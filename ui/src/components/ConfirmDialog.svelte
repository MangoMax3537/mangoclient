<script lang="ts">
  import WarningIcon from 'phosphor-svelte/lib/WarningIcon';
  import ClockCounterClockwiseIcon from 'phosphor-svelte/lib/ClockCounterClockwiseIcon';
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
    /** Label/value facts shown under the message (playtime, size ...): what the user is about to lose. */
    facts?: { label: string; value: string }[];
    /** A consequence the user must read, set apart from the message. */
    warn?: string;
    /** A neutral side effect the user should know about ("a snapshot is taken first"). */
    note?: string;
    onconfirm: () => void;
    oncancel: () => void;
  }
  let { title, body, confirmLabel, danger = false, busy = false, infoOnly = false, items = [], facts = [], warn = '', note = '', onconfirm, oncancel }: Props = $props();
</script>

<Modal {title} onclose={oncancel} onenter={infoOnly || busy ? undefined : onconfirm}>
  <p class="msg">{body}</p>
  {#if facts.length}
    <dl class="facts">{#each facts as f (f.label)}<div><dt>{f.label}</dt><dd class="mono">{f.value}</dd></div>{/each}</dl>
  {/if}
  {#if warn}<p class="warn" role="note"><WarningIcon weight="bold" size={16} aria-hidden="true" /><span>{warn}</span></p>{/if}
  {#if note}<p class="note hint" role="note"><ClockCounterClockwiseIcon weight="bold" size={14} aria-hidden="true" /><span>{note}</span></p>{/if}
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
  .facts { margin: 6px 0 10px; display: grid; gap: 0; border: 1px solid var(--line); border-radius: var(--r-m); }
  .facts div { display: flex; justify-content: space-between; gap: 16px; padding: 7px 12px; border-bottom: 1px solid var(--line-soft); }
  .facts div:last-child { border-bottom: 0; }
  dt { color: var(--paper-dim); }
  dd { margin: 0; }
  .warn { display: flex; gap: 8px; align-items: flex-start; margin: 8px 0 0; padding: 8px 12px; border-left: 3px solid var(--tomato); background: color-mix(in srgb, var(--tomato) 10%, transparent); font-size: var(--fs-sm); }
  .warn :global(svg) { flex: none; margin-top: 2px; color: var(--tomato); }
  .note { display: flex; gap: 6px; align-items: flex-start; margin: 10px 0 0; }
  .note :global(svg) { flex: none; margin-top: 3px; color: var(--paper-mute); }
  .items { margin: 8px 0 0; padding: 0; list-style: none; display: flex; flex-wrap: wrap; gap: 6px; }
  .items li { padding: 3px 9px; border: 1px solid var(--line); border-radius: var(--r-s); background: var(--ink-3); font-size: 13px; }
</style>
