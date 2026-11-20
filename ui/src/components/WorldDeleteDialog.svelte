<script lang="ts">
  import WarningIcon from 'phosphor-svelte/lib/WarningIcon';
  import Modal from './Modal.svelte';
  import BusyBar from './BusyBar.svelte';
  import { deleteDefaults, modeKey } from '../lib/backups';
  import { deleteWorld, wb } from '../lib/worlds.svelte';
  import { errorText, lang, t } from '../lib/stores.svelte';
  import { fmtBytes, fmtRelative } from '../lib/format';
  import type { Backup, World } from '../lib/types';

  interface Props { profileId: string; world: World; onclose: () => void; ondone: (made: Backup | null) => void }
  let { profileId, world, onclose, ondone }: Props = $props();
  let backupFirst = $state(deleteDefaults().backupFirst);
  let error = $state('');
  const busy = $derived(!!wb.busy[profileId]);
  const facts = $derived([
    { label: t('worlds.fact.size'), value: fmtBytes(world.sizeBytes, lang()) },
    { label: t('worlds.fact.played'), value: world.lastPlayed ? (fmtRelative(world.lastPlayed, lang()) ?? '') : t('worlds.neverPlayed') },
    ...(world.gameMode ? [{ label: t('worlds.fact.mode'), value: t(modeKey(world.gameMode)) }] : []),
    ...(world.version ? [{ label: t('worlds.fact.version'), value: world.version }] : []),
  ]);

  const gone = $derived(!!wb.worlds[profileId] && !wb.worlds[profileId]?.some((w) => w.id === world.id));
  const warnText = $derived(world.hardcore ? `${t('worlds.deleteWarn')} ${t('worlds.deleteHardcore')}` : t('worlds.deleteWarn'));

  async function go() {
    if (busy) return;
    error = '';
    try { ondone(await deleteWorld(profileId, world, backupFirst)); }
    catch (e) { error = errorText(e); }
  }
</script>

<Modal title={t('worlds.deleteTitle', { name: world.name })} {onclose} onenter={go} locked={busy}>
  <p class="msg">{t('worlds.deleteBody')}</p>
  <dl class="facts">{#each facts as f (f.label)}<div><dt>{f.label}</dt><dd class="mono">{f.value}</dd></div>{/each}</dl>
  <p class="warn" role="note"><WarningIcon weight="bold" size={16} aria-hidden="true" /><span>{warnText}</span></p>
  <label class="chk"><input type="checkbox" bind:checked={backupFirst} disabled={busy} />
    <span><b>{t('worlds.backupFirst')}</b><span class="hint">{t('worlds.backupFirstHint')}</span></span></label>
  {#if busy}<div class="b"><BusyBar label={t('worlds.deleteRunning', { name: world.name })} /></div>{/if}
  {#if error}<p class="err" role="alert">{error}</p>{/if}
  {#snippet footer()}
    <button class="btn ghost" type="button" disabled={busy} onclick={onclose}>{t('common.cancel')}</button>
    <button class="btn danger" type="button" disabled={busy || gone} onclick={go}>{backupFirst ? t('worlds.deleteConfirm') : t('worlds.deleteNoBackup')}</button>
  {/snippet}
</Modal>

<style>
  .msg { margin: 0 0 8px; color: var(--paper-dim); }
  .facts { margin: 6px 0 10px; display: grid; border: 1px solid var(--line); border-radius: var(--r-m); }
  .facts div { display: flex; justify-content: space-between; gap: 16px; padding: 7px 12px; border-bottom: 1px solid var(--line-soft); }
  .facts div:last-child { border-bottom: 0; }
  dt { color: var(--paper-dim); }
  dd { margin: 0; }
  .warn { display: flex; gap: 8px; align-items: flex-start; margin: 8px 0 14px; padding: 8px 12px; border-left: 3px solid var(--tomato); background: color-mix(in srgb, var(--tomato) 10%, transparent); font-size: var(--fs-sm); }
  .warn :global(svg) { flex: none; margin-top: 2px; color: var(--tomato); }
  .chk { display: flex; gap: 10px; align-items: flex-start; cursor: pointer; padding: 10px 12px; border: 1px solid var(--line); border-radius: var(--r-m); }
  .chk input { margin-top: 3px; width: 16px; height: 16px; flex: none; }
  .chk span { display: grid; gap: 2px; }
  .b { margin-top: 14px; }
  .err { margin: 12px 0 0; padding: 8px 12px; border-left: 3px solid var(--tomato); background: color-mix(in srgb, var(--tomato) 10%, transparent); font-size: var(--fs-sm); }
</style>
