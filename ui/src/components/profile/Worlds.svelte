<script lang="ts">
  import FolderOpenIcon from 'phosphor-svelte/lib/FolderOpenIcon';
  import TrashIcon from 'phosphor-svelte/lib/TrashIcon';
  import FloppyDiskIcon from 'phosphor-svelte/lib/FloppyDiskIcon';
  import SkullIcon from 'phosphor-svelte/lib/SkullIcon';
  import SwordIcon from 'phosphor-svelte/lib/SwordIcon';
  import PaintBrushIcon from 'phosphor-svelte/lib/PaintBrushIcon';
  import CompassIcon from 'phosphor-svelte/lib/CompassIcon';
  import EyeIcon from 'phosphor-svelte/lib/EyeIcon';
  import CubeIcon from 'phosphor-svelte/lib/CubeIcon';
  import WarningIcon from 'phosphor-svelte/lib/WarningIcon';
  import PlayIcon from 'phosphor-svelte/lib/PlayIcon';
  import EmptyState from '../EmptyState.svelte';
  import Tile from '../Tile.svelte';
  import WorldCardSkeleton from '../skeleton/WorldCardSkeleton.svelte';
  import { BusyGate } from '../../lib/loadState.svelte';
  import BusyBar from '../BusyBar.svelte';
  import BackupDialog from '../BackupDialog.svelte';
  import WorldDeleteDialog from '../WorldDeleteDialog.svelte';
  import { modeKey, versionDiffers } from '../../lib/backups';
  import { fmtBytes, fmtRelative } from '../../lib/format';
  import { loadWorlds, wb } from '../../lib/worlds.svelte';
  import { actions, app, errorText, getApi, isBusy, lang, t, toast, tp } from '../../lib/stores.svelte';
  import type { Backup, GameMode, ProfileView, World } from '../../lib/types';

  /** Nothing to show: render nothing at all (the Media tab then shows one shared empty state instead). */
  let { profile, hideWhenEmpty = false }: { profile: ProfileView; hideWhenEmpty?: boolean } = $props();
  let failed = $state(false);
  let backupOf = $state<World | null>(null);
  let deleteOf = $state<World | null>(null);
  const worlds = $derived(wb.worlds[profile.id]);
  const running = $derived(isBusy(profile.id));
  const pending = $derived(wb.busy[profile.id]);
  const MODE_ICON: Record<GameMode, typeof SwordIcon> = { survival: SwordIcon, creative: PaintBrushIcon, adventure: CompassIcon, spectator: EyeIcon };

  $effect(() => {
    const id = profile.id;
    let dead = false;
    failed = false;
    loadWorlds(id).catch((e) => { if (!dead) { failed = true; wb.worlds[id] = []; toast(errorText(e), 'error'); } });
    return () => { dead = true; };
  });

  const gate = new BusyGate();
  $effect(() => { gate.update(worlds === undefined); return () => gate.destroy(); });

  async function reveal() {
    try { toast(t('worlds.folderOpened', { path: await getApi().worldOpenFolder({ profileId: profile.id }) }), 'info'); } catch (e) { toast(errorText(e), 'error'); }
  }
  function backedUp(b: Backup) {
    backupOf = null;
    toast(t('worlds.backupDone', { name: b.worldId ?? '', size: fmtBytes(b.sizeBytes, lang()) }), 'ok');
  }
  function deleted(w: World, made: Backup | null) {
    deleteOf = null;
    toast(t(made ? 'worlds.deletedBacked' : 'worlds.deleted', { name: w.name }), 'ok');
  }
</script>

{#if !(hideWhenEmpty && worlds !== undefined && worlds.length === 0)}
<section class="ws" aria-labelledby="ws-h">
  <header>
    <h2 id="ws-h" class="eyebrow">{t('worlds.title')}</h2>
    {#if worlds?.length}<span class="hint mono">{tp('worlds.count', worlds.length)}</span>{/if}
  </header>

  {#if running && worlds?.length}
    <p class="note" id="ws-run" role="note"><WarningIcon weight="bold" size={16} aria-hidden="true" /><span>{t('worlds.runningNote')}</span></p>
  {/if}
  {#if pending}<div class="pend"><BusyBar label={pending.kind === 'snapshot' ? t('backups.busy.snapshot') : pending.kind === 'backup' ? t('worlds.backupRunning', { name: pending.name ?? '' }) : pending.kind === 'delete' ? t('worlds.deleteRunning', { name: pending.name ?? '' }) : t('backups.busy.restore')} /></div>{/if}

  {#if worlds === undefined}
    {#if gate.show}<WorldCardSkeleton count={2} />{/if}
  {:else if worlds.length === 0}
    <EmptyState title={t('worlds.emptyTitle')} hint={failed ? '' : t('worlds.empty')}>
      <button class="btn accent" type="button" disabled={!app.accounts.length || running} onclick={() => actions.play(profile.id)}><PlayIcon weight="bold" size={16} aria-hidden="true" />{t('home.play')}</button>
    </EmptyState>
  {:else}
    <ul class="grid">
      {#each worlds as w (w.id)}
        {@const Mode = w.gameMode ? MODE_ICON[w.gameMode] : null}
        <li class="card">
          <Tile kind="world" id={w.id} src={w.iconUrl} size={64} notch={10} />
          <div class="info">
            <h3 title={w.name}>{w.name}</h3>
            <ul class="chips" aria-label={w.name}>
              {#if w.gameMode && Mode}<li class="chip"><Mode weight="bold" size={13} aria-hidden="true" />{t(modeKey(w.gameMode))}</li>{/if}
              {#if w.hardcore}<li class="chip hc"><SkullIcon weight="bold" size={13} aria-hidden="true" />{t('worlds.hardcore')}</li>{/if}
              {#if w.version}<li class="chip mono" class:diff={versionDiffers(w.version, profile.mcVersion)}><CubeIcon weight="bold" size={13} aria-hidden="true" />{w.version}</li>{/if}
            </ul>
            {#if w.id !== w.name}<p class="folder mono" title={w.id}>{w.id}</p>{/if}
            <p class="meta mono">{fmtBytes(w.sizeBytes, lang())} &middot; {w.lastPlayed ? t('worlds.played', { when: fmtRelative(w.lastPlayed, lang()) ?? '' }) : t('worlds.neverPlayed')}</p>
            {#if versionDiffers(w.version, profile.mcVersion)}<p class="hint vd">{t('worlds.versionDiff', { v: w.version ?? '', pv: profile.mcVersion })}</p>{/if}
          </div>
          <div class="acts">
            <button class="btn small" type="button" disabled={!!pending} onclick={() => (backupOf = w)}><FloppyDiskIcon weight="bold" size={14} aria-hidden="true" />{t('worlds.backup')}</button>
            <button class="btn small ghost" type="button" onclick={reveal}><FolderOpenIcon weight="bold" size={14} aria-hidden="true" />{t('worlds.openFolder')}</button>
            <button class="btn small danger" type="button" disabled={running || !!pending} aria-describedby={running ? 'ws-run' : undefined} onclick={() => (deleteOf = w)}><TrashIcon weight="bold" size={14} aria-hidden="true" />{t('worlds.delete')}</button>
          </div>
        </li>
      {/each}
    </ul>
  {/if}
</section>
{/if}

{#if backupOf}
  <BackupDialog profileId={profile.id} world={backupOf} onclose={() => (backupOf = null)} ondone={backedUp} />
{/if}
{#if deleteOf}
  {@const w = deleteOf}
  <WorldDeleteDialog profileId={profile.id} world={w} onclose={() => (deleteOf = null)} ondone={(m) => deleted(w, m)} />
{/if}

<style>
  .ws { margin-bottom: var(--s-6); }
  header { display: flex; align-items: baseline; justify-content: space-between; margin-bottom: 10px; }
  .note { display: flex; gap: 8px; align-items: flex-start; margin: 0 0 10px; padding: 8px 12px; border: 1px solid color-mix(in srgb, var(--mango) 45%, transparent); border-radius: var(--r-m); background: color-mix(in srgb, var(--mango) 10%, transparent); font-size: var(--fs-sm); }
  .note :global(svg) { flex: none; margin-top: 2px; color: var(--mango); }
  .pend { margin-bottom: 10px; }
  .grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(min(100%, 420px), 1fr)); gap: 10px; list-style: none; margin: 0; padding: 0; }
  .card { display: grid; grid-template-columns: auto minmax(0, 1fr); gap: 4px 14px; padding: 12px; border: 1px solid var(--line); border-radius: var(--r-m); background: var(--ink-2); }
  .info { display: grid; gap: 6px; min-width: 0; align-content: start; }
  h3 { font-size: var(--fs-lg); font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .chips { display: flex; flex-wrap: wrap; gap: 6px; list-style: none; margin: 0; padding: 0; }
  .chip { height: 22px; font-size: var(--fs-xs); }
  .chip.hc { color: var(--tomato); border-color: var(--tomato); }
  .chip.hc :global(svg) { color: var(--tomato); }
  .chip.diff { color: var(--mango); border-color: var(--mango); }
  .meta { margin: 0; font-size: var(--fs-sm); color: var(--paper-dim); }
  .vd { margin: 0; }
  .folder { margin: 0; font-size: var(--fs-xs); color: var(--paper-mute); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .acts { grid-column: 1 / -1; display: flex; flex-wrap: wrap; gap: 6px; padding-top: 8px; border-top: 1px solid var(--line-soft); margin-top: 4px; }
  .acts .danger { margin-left: auto; }
</style>
