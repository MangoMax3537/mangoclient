<script lang="ts">
  import WarningIcon from 'phosphor-svelte/lib/WarningIcon';
  import Modal from './Modal.svelte';
  import BusyBar from './BusyBar.svelte';
  import { backupTitle, worldStillExists, type RestoreMode } from '../lib/backups';
  import { restoreBackup, wb } from '../lib/worlds.svelte';
  import { errorText, lang, t } from '../lib/stores.svelte';
  import { fmtDateTime } from '../lib/format';
  import type { Backup } from '../lib/types';

  /** World backup: "Restore" (never overwrites) or "Restore as new copy". Snapshot: a confirm that says it replaces content. */
  interface Props { profileId: string; backup: Backup; onclose: () => void; ondone: (mode: RestoreMode) => void }
  let { profileId, backup, onclose, ondone }: Props = $props();
  let mode = $state<RestoreMode>('default');
  let error = $state('');
  const busy = $derived(!!wb.busy[profileId]);
  const snap = $derived(backup.kind === 'snapshot');
  const exists = $derived(worldStillExists(backup, wb.worlds[profileId]));
  /** A stale-list error refreshed the lists and this backup is gone: confirming again can only fail the same way. */
  const gone = $derived(!!wb.backups[profileId] && !wb.backups[profileId]?.some((b) => b.id === backup.id));
  const name = $derived(backup.worldId ?? backupTitle(backup));

  async function go() {
    if (busy) return;
    error = '';
    try { await restoreBackup(profileId, backup, mode); ondone(mode); }
    catch (e) { error = errorText(e); }
  }
</script>

<Modal title={snap ? t('backups.restoreSnapTitle') : t('backups.restoreWorldTitle', { name })} {onclose} onenter={go} locked={busy}>
  {#if snap}
    <p class="msg">{t('backups.restoreSnapBody', { date: fmtDateTime(backup.createdAt, lang()) })}</p>
    <p class="warn" role="note"><WarningIcon weight="bold" size={16} aria-hidden="true" /><span>{t('backups.restoreSnapWarn')}</span></p>
  {:else}
    <p class="msg">{t('backups.restoreWorldBody')}</p>
    <div class="opts" role="radiogroup" aria-label={t('backups.restoreWorldTitle', { name })}>
      <label class:on={mode === 'default'}><input type="radio" name="rmode" value="default" bind:group={mode} disabled={busy} />
        <span><b>{t('backups.mode.default')}</b><span class="hint">{exists ? t('backups.mode.default.exists', { name }) : t('backups.mode.default.free', { name })}</span></span></label>
      <label class:on={mode === 'asNew'}><input type="radio" name="rmode" value="asNew" bind:group={mode} disabled={busy} />
        <span><b>{t('backups.mode.asNew')}</b><span class="hint">{t('backups.mode.asNew.hint')}</span></span></label>
    </div>
  {/if}
  {#if busy}<div class="b"><BusyBar label={t('backups.busy.restore')} /></div>{/if}
  {#if error}<p class="err" role="alert">{error}</p>{/if}
  {#snippet footer()}
    <button class="btn ghost" type="button" disabled={busy} onclick={onclose}>{t('common.cancel')}</button>
    <button class="btn" class:danger={snap} class:accent={!snap} type="button" disabled={busy || gone} onclick={go}>
      {snap ? t('backups.restoreSnapConfirm') : mode === 'asNew' ? t('backups.restoreCopyConfirm') : t('backups.restoreWorldConfirm')}
    </button>
  {/snippet}
</Modal>

<style>
  .msg { margin: 0 0 10px; color: var(--paper-dim); }
  .warn { display: flex; gap: 8px; align-items: flex-start; margin: 8px 0 0; padding: 8px 12px; border: 1px solid color-mix(in srgb, var(--tomato) 45%, transparent); border-radius: var(--r-m); background: color-mix(in srgb, var(--tomato) 10%, transparent); font-size: var(--fs-sm); }
  .warn :global(svg) { flex: none; margin-top: 2px; color: var(--tomato); }
  .opts { display: grid; gap: 8px; }
  .opts label { display: flex; gap: 10px; align-items: flex-start; cursor: pointer; padding: 10px 12px; border: 1px solid var(--line); border-radius: var(--r-m); }
  .opts label.on { border-color: var(--mango); }
  .opts input { margin-top: 3px; width: 16px; height: 16px; flex: none; }
  .opts span { display: grid; gap: 2px; }
  .b { margin-top: 14px; }
  .err { margin: 12px 0 0; padding: 8px 12px; border: 1px solid color-mix(in srgb, var(--tomato) 45%, transparent); border-radius: var(--r-m); background: color-mix(in srgb, var(--tomato) 10%, transparent); font-size: var(--fs-sm); }
</style>
