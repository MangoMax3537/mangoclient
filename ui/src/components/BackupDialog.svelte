<script lang="ts">
  import Modal from './Modal.svelte';
  import BusyBar from './BusyBar.svelte';
  import { LABEL_MAX } from '../lib/backups';
  import { backupWorld, createSnapshot, wb } from '../lib/worlds.svelte';
  import { errorText, t } from '../lib/stores.svelte';
  import type { Backup, World } from '../lib/types';

  /** Back up one world (`world` set) or create a snapshot of the instance (`world` null), with an optional label. */
  interface Props { profileId: string; world: World | null; onclose: () => void; ondone: (b: Backup) => void }
  let { profileId, world, onclose, ondone }: Props = $props();
  let label = $state('');
  let error = $state('');
  const busy = $derived(!!wb.busy[profileId]);

  async function go() {
    if (busy) return;
    error = '';
    try { ondone(world ? await backupWorld(profileId, world, label) : await createSnapshot(profileId, label)); }
    catch (e) { error = errorText(e); }
  }
  const title = $derived(world ? t('worlds.backupTitle', { name: world.name }) : t('backups.snapshotTitle'));
  const body = $derived(world ? t('worlds.backupBody') : t('backups.snapshotBody'));
</script>

<Modal {title} {onclose} onenter={go} locked={busy}>
  <p class="msg">{body}</p>
  <div class="f">
    <label for="bk-label">{t('worlds.labelField')}</label>
    <input id="bk-label" class="field" maxlength={LABEL_MAX} placeholder={t('worlds.labelPlaceholder')} bind:value={label} disabled={busy} autocomplete="off" spellcheck="false" />
  </div>
  {#if busy}<div class="b"><BusyBar label={world ? t('worlds.backupRunning', { name: world.name }) : t('backups.busy.snapshot')} /></div>{/if}
  {#if error}<p class="err" role="alert">{error}</p>{/if}
  {#snippet footer()}
    <button class="btn ghost" type="button" disabled={busy} onclick={onclose}>{t('common.cancel')}</button>
    <button class="btn accent" type="button" disabled={busy} onclick={go}>{world ? t('worlds.backupConfirm') : t('backups.snapshotConfirm')}</button>
  {/snippet}
</Modal>

<style>
  .msg { margin: 0 0 12px; color: var(--paper-dim); }
  .f { display: grid; gap: 6px; }
  .f label { font-weight: 500; }
  .b { margin-top: 14px; }
  .err { margin: 12px 0 0; padding: 8px 12px; border: 1px solid color-mix(in srgb, var(--tomato) 45%, transparent); background: color-mix(in srgb, var(--tomato) 10%, transparent); font-size: var(--fs-sm); }
</style>
