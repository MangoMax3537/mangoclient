// State and actions for worlds, backups and snapshots, per instance. The backend owns the truth: every mutation returns or triggers a
// reload, and a stale-list error (backup_not_found, world_not_found) refreshes the lists. Errors are thrown to the caller, which shows
// the localized sentence where the user is looking (inline in a dialog, or a toast).
import { toAppError } from './api';
import { sortBackups, STALE_CODES, cleanLabel, deleteArgs, restoreArgs, type RestoreMode } from './backups';
import { getApi } from './stores.svelte';
import type { Backup, World } from './types';

export type BusyKind = 'snapshot' | 'backup' | 'restore' | 'delete';
export const wb = $state({
  worlds: {} as Record<string, World[] | undefined>,
  backups: {} as Record<string, Backup[] | undefined>,
  busy: {} as Record<string, { kind: BusyKind; name?: string } | undefined>,
});

export function resetWorlds(): void {
  for (const k of Object.keys(wb.worlds)) delete wb.worlds[k];
  for (const k of Object.keys(wb.backups)) delete wb.backups[k];
  for (const k of Object.keys(wb.busy)) delete wb.busy[k];
}

export async function loadWorlds(profileId: string): Promise<World[]> {
  const list = await getApi().worldsList({ profileId });
  wb.worlds[profileId] = list;
  return list;
}
export async function loadBackups(profileId: string): Promise<Backup[]> {
  const list = sortBackups(await getApi().backupsList({ profileId }));
  wb.backups[profileId] = list;
  return list;
}
/** Reloads whatever a failed call may have made stale. Never throws. */
async function refreshAfterError(profileId: string, e: unknown): Promise<void> {
  if (!STALE_CODES.includes(toAppError(e).code)) return;
  await Promise.allSettled([loadWorlds(profileId), loadBackups(profileId)]);
}

async function run<T>(profileId: string, kind: BusyKind, name: string | undefined, fn: () => Promise<T>): Promise<T> {
  if (wb.busy[profileId]) throw { code: 'backup_busy', message: 'busy', needsRelogin: false }; // never two zips at once; the UI disables the buttons first
  wb.busy[profileId] = { kind, name };
  try { return await fn(); } catch (e) { await refreshAfterError(profileId, e); throw e; } finally { delete wb.busy[profileId]; }
}

export const backupWorld = (profileId: string, world: World, label: string): Promise<Backup> =>
  run(profileId, 'backup', world.name, async () => {
    const b = await getApi().worldBackup({ profileId, worldId: world.id, label: cleanLabel(label) ?? null });
    await loadBackups(profileId);
    return b;
  });

/** Returns the backup that was made first, if any. */
export const deleteWorld = (profileId: string, world: World, backupFirst: boolean): Promise<Backup | null> =>
  run(profileId, 'delete', world.name, async () => {
    const b = await getApi().worldDelete(deleteArgs(profileId, world, backupFirst));
    await Promise.all([loadWorlds(profileId), loadBackups(profileId)]);
    return b;
  });

export const createSnapshot = (profileId: string, label: string): Promise<Backup> =>
  run(profileId, 'snapshot', undefined, async () => {
    const b = await getApi().snapshotCreate({ profileId, label: cleanLabel(label) ?? null });
    await loadBackups(profileId);
    return b;
  });

export const restoreBackup = (profileId: string, b: Backup, mode: RestoreMode): Promise<void> =>
  run(profileId, 'restore', b.label || b.worldId || undefined, async () => {
    wb.backups[profileId] = sortBackups(await getApi().backupRestore(restoreArgs(profileId, b, mode)));
    await loadWorlds(profileId); // a world restore adds a folder; a snapshot restore does not touch worlds, but a reload is cheap
  });

export const deleteBackup = (profileId: string, b: Backup): Promise<void> =>
  run(profileId, 'delete', b.label || undefined, async () => {
    wb.backups[profileId] = sortBackups(await getApi().backupDelete({ profileId, backupId: b.id }));
  });
