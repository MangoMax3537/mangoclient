<script lang="ts">
  import ArrowCounterClockwiseIcon from 'phosphor-svelte/lib/ArrowCounterClockwiseIcon';
  import TrashIcon from 'phosphor-svelte/lib/TrashIcon';
  import CameraIcon from 'phosphor-svelte/lib/CameraIcon';
  import WarningIcon from 'phosphor-svelte/lib/WarningIcon';
  import EmptyState from '../EmptyState.svelte';
  import Skeleton from '../skeleton/Skeleton.svelte';
  import BackupRowSkeleton from '../skeleton/BackupRowSkeleton.svelte';
  import { BusyGate } from '../../lib/loadState.svelte';
  import Tile from '../Tile.svelte';
  import BusyBar from '../BusyBar.svelte';
  import ConfirmDialog from '../ConfirmDialog.svelte';
  import BackupDialog from '../BackupDialog.svelte';
  import RestoreDialog from '../RestoreDialog.svelte';
  import { backupTitle, reasonKey } from '../../lib/backups';
  import { fmtBytes, fmtDateTime } from '../../lib/format';
  import { deleteBackup, loadBackups, loadWorlds, wb } from '../../lib/worlds.svelte';
  import { errorText, isBusy, lang, t, toast, tp } from '../../lib/stores.svelte';
  import type { Backup, ProfileView } from '../../lib/types';

  let { profile }: { profile: ProfileView } = $props();
  let snapshotOpen = $state(false);
  let restoreOf = $state<Backup | null>(null);
  let deleteOf = $state<Backup | null>(null);
  let deleteError = $state('');
  const list = $derived(wb.backups[profile.id]);
  const gate = new BusyGate();
  $effect(() => { gate.update(list === undefined); return () => gate.destroy(); });
  const worlds = $derived(wb.worlds[profile.id]);
  const running = $derived(isBusy(profile.id));
  const pending = $derived(wb.busy[profile.id]);

  $effect(() => {
    const id = profile.id;
    let dead = false;
    loadBackups(id).catch((e) => { if (!dead) { wb.backups[id] = []; toast(errorText(e), 'error'); } });
    if (wb.worlds[id] === undefined) loadWorlds(id).catch(() => {}); // only used to tell the restore dialog whether the world still exists
    return () => { dead = true; };
  });

  const iconFor = (b: Backup): string | null => (b.worldId ? (worlds?.find((w) => w.id === b.worldId)?.iconUrl ?? null) : null);
  const titleOf = (b: Backup) => backupTitle(b) || t('backups.unnamedSnapshot');

  async function confirmDelete() {
    const b = deleteOf;
    if (!b) return;
    deleteError = '';
    try { await deleteBackup(profile.id, b); deleteOf = null; toast(t('backups.deleted'), 'ok'); }
    catch (e) { deleteError = errorText(e); }
  }
</script>

<section class="bk" aria-labelledby="bk-h">
  <header>
    <div class="h"><h2 id="bk-h" class="eyebrow">{t('backups.title')}</h2>{#if list?.length}<span class="hint mono">{tp('backups.count', list.length)}</span>{/if}</div>
    <button class="btn accent" type="button" disabled={!!pending} onclick={() => (snapshotOpen = true)}><CameraIcon weight="bold" size={16} aria-hidden="true" />{t('backups.snapshot')}</button>
  </header>

  {#if running && list?.length}
    <p class="note" id="bk-run" role="note"><WarningIcon weight="bold" size={16} aria-hidden="true" /><span>{t('backups.runningNote')}</span></p>
  {/if}

  {#if list === undefined}
    {#if gate.show}<BackupRowSkeleton count={4} />{/if}
  {:else if list.length === 0 && !pending}
    <EmptyState title={t('backups.emptyTitle')} hint={t('backups.empty')}>
      <button class="btn accent" type="button" onclick={() => (snapshotOpen = true)}><CameraIcon weight="bold" size={16} aria-hidden="true" />{t('backups.snapshot')}</button>
    </EmptyState>
  {:else}
    <ul class="rows">
      {#if pending}
        <li class="row pend"><Skeleton variant="box" w={44} h={44} notch={8} /><BusyBar label={t(`backups.busy.${pending.kind}`)} /></li>
      {/if}
      {#each list as b (b.id)}
        <li class="row">
          <Tile kind="world" id={b.worldId ?? b.id} src={iconFor(b)} size={44} notch={8} />
          <div class="t">
            <b title={titleOf(b)}>{titleOf(b)}</b>
            <div class="chips">
              <span class="chip" class:snap={b.kind === 'snapshot'}>{t(`backups.kind.${b.kind}`)}</span>
              <span class="chip">{t(reasonKey(b.reason))}</span>
              {#if b.kind === 'world' && b.worldId && b.label && b.label !== b.worldId}<span class="chip mono">{b.worldId}</span>{/if}
            </div>
          </div>
          <div class="when mono"><span>{fmtDateTime(b.createdAt, lang())}</span><span class="sz">{fmtBytes(b.sizeBytes, lang())}</span></div>
          <div class="acts">
            <button class="btn small" type="button" disabled={running || !!pending} aria-describedby={running ? 'bk-run' : undefined} onclick={() => (restoreOf = b)}><ArrowCounterClockwiseIcon weight="bold" size={14} aria-hidden="true" />{t('backups.restore')}</button>
            <button class="btn small ghost danger" type="button" disabled={!!pending} aria-label={`${t('backups.delete')}: ${titleOf(b)}`} onclick={() => { deleteError = ''; deleteOf = b; }}><TrashIcon weight="bold" size={14} aria-hidden="true" />{t('backups.delete')}</button>
          </div>
        </li>
      {/each}
    </ul>
  {/if}
</section>

{#if snapshotOpen}
  <BackupDialog profileId={profile.id} world={null} onclose={() => (snapshotOpen = false)} ondone={(b) => { snapshotOpen = false; toast(t('backups.snapshotDone', { size: fmtBytes(b.sizeBytes, lang()) }), 'ok'); }} />
{/if}
{#if restoreOf}
  {@const b = restoreOf}
  <RestoreDialog profileId={profile.id} backup={b} onclose={() => (restoreOf = null)}
    ondone={() => { restoreOf = null; toast(b.kind === 'snapshot' ? t('backups.restoredSnap') : t('backups.restored', { name: titleOf(b) }), 'ok', 7000); }} />
{/if}
{#if deleteOf}
  <ConfirmDialog danger title={t('backups.deleteTitle')} body={t('backups.deleteBody', { name: titleOf(deleteOf), size: fmtBytes(deleteOf.sizeBytes, lang()) })}
    facts={[{ label: t('backups.fact.created'), value: fmtDateTime(deleteOf.createdAt, lang()) }]} warn={deleteError} confirmLabel={t('backups.deleteConfirm')}
    busy={!!pending} onconfirm={confirmDelete} oncancel={() => (deleteOf = null)} />
{/if}

<style>
  header { display: flex; align-items: center; justify-content: space-between; gap: 12px; margin-bottom: 12px; flex-wrap: wrap; }
  .h { display: flex; align-items: baseline; gap: 12px; }
  .note { display: flex; gap: 8px; align-items: flex-start; margin: 0 0 10px; padding: 8px 12px; border-left: 3px solid var(--mango); background: color-mix(in srgb, var(--mango) 10%, transparent); font-size: var(--fs-sm); }
  .note :global(svg) { flex: none; margin-top: 2px; color: var(--mango); }
  .rows { list-style: none; margin: 0; padding: 0; border: 1px solid var(--line); border-radius: var(--r-m); background: var(--ink-2); }
  .row { display: grid; grid-template-columns: auto minmax(0, 1fr) auto auto; gap: 14px; align-items: center; padding: 10px 12px; border-bottom: 1px solid var(--line-soft); }
  .row:last-child { border-bottom: 0; }
  .row.pend { grid-template-columns: auto minmax(0, 1fr); }
  .t { display: grid; gap: 5px; min-width: 0; }
  .t b { font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .chips { display: flex; flex-wrap: wrap; gap: 6px; }
  .chip { height: 20px; padding: 0 7px; font-size: var(--fs-xs); }
  .chip.snap { color: var(--sky); border-color: var(--sky); }
  .when { display: grid; gap: 2px; justify-items: end; font-size: var(--fs-sm); color: var(--paper-dim); white-space: nowrap; }
  .acts { display: flex; gap: 6px; }
  @media (max-width: 1040px) {
    .row { grid-template-columns: auto minmax(0, 1fr) auto; }
    .when { grid-column: 2; grid-row: 2; display: flex; gap: 12px; }
    .acts { grid-column: 3; grid-row: 1 / span 2; flex-direction: column; }
  }
</style>
