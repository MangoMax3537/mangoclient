// Pure rules for worlds, backups and snapshots (no Svelte, no API): defaults of the dialogs, ordering, labels and validation.
import type { Key } from './i18n';
import type { Backup, BackupReason, GameMode, World, WorldDeleteArgs, BackupRestoreArgs } from './types';

export const BACKUP_KEEP_MIN = 1;
export const BACKUP_KEEP_MAX = 50;
export const LABEL_MAX = 60;

/** Error codes of the backup commands that carry their own localized sentence (`err.<code>`). */
export const BACKUP_ERROR_CODES = [
  'game_running', 'backup_corrupt', 'backup_not_found', 'world_not_found', 'invalid_world', 'backup_too_large', 'restore_name_taken',
] as const;
/** Codes after which the lists are out of date and must be reloaded. */
export const STALE_CODES: readonly string[] = ['backup_not_found', 'world_not_found'];

/** The label the UI sends: trimmed, bounded, `undefined` when empty (the backend then names the backup itself). */
export function cleanLabel(raw: string): string | undefined {
  const s = raw.replace(/[\u0000-\u001f]/g, ' ').trim().slice(0, LABEL_MAX).trim();
  return s ? s : undefined;
}

/** Newest first; ties broken by id so the order is stable. */
export const sortBackups = (list: readonly Backup[]): Backup[] =>
  [...list].sort((a, b) => b.createdAt - a.createdAt || (a.id < b.id ? 1 : -1));

/** The "Delete world" dialog: backing up first is ON unless the user turns it off. */
export const deleteDefaults = (): { backupFirst: boolean } => ({ backupFirst: true });
export const deleteArgs = (profileId: string, world: World, backupFirst: boolean): WorldDeleteArgs =>
  ({ profileId, worldId: world.id, makeBackup: backupFirst });

export type RestoreMode = 'default' | 'asNew';
/** Snapshots always replace and never carry `asNew`; a world restore sends `asNew` only when the user chose the copy. */
export function restoreArgs(profileId: string, b: Backup, mode: RestoreMode): BackupRestoreArgs {
  if (b.kind === 'snapshot') return { profileId, backupId: b.id };
  return mode === 'asNew' ? { profileId, backupId: b.id, asNew: true } : { profileId, backupId: b.id };
}
/** Whether a world with this backup's folder name is still in the instance (decides which hint the default option shows). */
export const worldStillExists = (b: Backup, worlds: readonly World[] | undefined): boolean =>
  !!b.worldId && !!worlds?.some((w) => w.id === b.worldId);

/** What a backup row is called: its label, else the world folder, else "Snapshot". */
export const backupTitle = (b: Backup): string => b.label || b.worldId || '';

export const reasonKey = (r: BackupReason): Key => `backups.reason.${r}` as Key;
export const modeKey = (m: GameMode): Key => `worlds.mode.${m}` as Key;

/** Parses the "backups to keep" field: a whole number in 1..50, else `null`. */
export function parseKeep(text: string): number | null {
  const s = text.trim();
  if (!/^\d{1,3}$/.test(s)) return null;
  const n = Number(s);
  return n >= BACKUP_KEEP_MIN && n <= BACKUP_KEEP_MAX ? n : null;
}
export const clampKeep = (n: number): number => Math.min(BACKUP_KEEP_MAX, Math.max(BACKUP_KEEP_MIN, Math.round(n)));

/** True when the world was last saved with another game version than the instance runs. */
export function versionDiffers(world: string | null, instance: string): boolean {
  return !!world && world !== instance;
}
