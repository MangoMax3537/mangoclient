import { beforeEach, describe, expect, it } from 'vitest';
import { BACKUP_ERROR_CODES, clampKeep, cleanLabel, deleteArgs, deleteDefaults, LABEL_MAX, parseKeep, restoreArgs, sortBackups, worldStillExists } from './backups';
import { createMockApi } from './mockApi';
import { dictionaries, hasKey } from './i18n';
import { app, errorText, init, resetStores, t } from './stores.svelte';
import { backupWorld, createSnapshot, deleteBackup, deleteWorld, loadBackups, loadWorlds, resetWorlds, restoreBackup, wb } from './worlds.svelte';
import type { Backup, World } from './types';

const bk = (p: Partial<Backup>): Backup => ({ id: 'x', kind: 'world', profileId: 'p', worldId: 'W', createdAt: 1, sizeBytes: 1, label: '', reason: 'manual', ...p });
const world = (p: Partial<World> = {}): World => ({ id: 'W', name: 'W', lastPlayed: null, sizeBytes: 1, gameMode: 'survival', hardcore: false, version: null, iconUrl: null, ...p });

describe('backup rules', () => {
  it('delete dialog backs up first by default and sends makeBackup accordingly', () => {
    expect(deleteDefaults().backupFirst).toBe(true);
    expect(deleteArgs('p', world({ id: 'Iron' }), true)).toEqual({ profileId: 'p', worldId: 'Iron', makeBackup: true });
    expect(deleteArgs('p', world(), false).makeBackup).toBe(false);
  });
  it('world restore never sends asNew unless asked; snapshots never do', () => {
    expect(restoreArgs('p', bk({}), 'default')).toEqual({ profileId: 'p', backupId: 'x' });
    expect(restoreArgs('p', bk({}), 'asNew')).toEqual({ profileId: 'p', backupId: 'x', asNew: true });
    expect(restoreArgs('p', bk({ kind: 'snapshot', worldId: null }), 'asNew')).toEqual({ profileId: 'p', backupId: 'x' });
  });
  it('sorts newest first and detects an existing world', () => {
    expect(sortBackups([bk({ id: 'a', createdAt: 1 }), bk({ id: 'b', createdAt: 3 }), bk({ id: 'c', createdAt: 2 })]).map((b) => b.id)).toEqual(['b', 'c', 'a']);
    expect(worldStillExists(bk({}), [world()])).toBe(true);
    expect(worldStillExists(bk({}), [world({ id: 'other' })])).toBe(false);
    expect(worldStillExists(bk({ worldId: null }), [world()])).toBe(false);
  });
  it('cleans labels', () => {
    expect(cleanLabel('   ')).toBeUndefined();
    expect(cleanLabel('  a\nb ')).toBe('a b');
    expect(cleanLabel('x'.repeat(200))).toHaveLength(LABEL_MAX);
  });
  it('validates backupKeep as a whole number 1..50', () => {
    for (const ok of ['1', '5', '50', ' 12 ']) expect(parseKeep(ok), ok).not.toBeNull();
    for (const bad of ['', '0', '51', '-1', '2.5', 'abc', '1e1', '1000']) expect(parseKeep(bad), bad).toBeNull();
    expect(clampKeep(0)).toBe(1); expect(clampKeep(99)).toBe(50); expect(clampKeep(7.4)).toBe(7);
  });
});

describe('error mapping', () => {
  beforeEach(() => { resetStores(); });
  it('every backup error code has its own localized sentence in both languages, never the generic one', () => {
    for (const code of [...BACKUP_ERROR_CODES, 'backup_busy']) {
      for (const lang of ['de', 'en'] as const) {
        expect(hasKey(lang, `err.${code}`), `${lang} ${code}`).toBe(true);
        expect(dictionaries[lang][`err.${code}`]).not.toBe(dictionaries[lang]['err.unknown']);
      }
    }
  });
  it('errorText resolves a code from an AppError', () => {
    expect(errorText({ code: 'game_running', message: 'raw backend text', needsRelogin: false })).not.toContain('raw backend');
    expect(errorText({ code: 'backup_corrupt', message: 'x', needsRelogin: false })).toBe(t('err.backup_corrupt' as never));
  });
});

describe('worlds and backups store against the mock', () => {
  async function setup(opts = {}) {
    resetStores(); resetWorlds();
    const api = createMockApi({ timeScale: 0, ...opts });
    await init(api);
    return { api, id: app.profiles[0].id };
  }

  it('loads the seeded worlds and backups newest first', async () => {
    const { id } = await setup();
    const ws = await loadWorlds(id);
    expect(ws).toHaveLength(4);
    expect(ws.some((w) => w.hardcore)).toBe(true);
    expect(ws.some((w) => w.gameMode === 'creative')).toBe(true);
    const bs = await loadBackups(id);
    const times = bs.map((b) => b.createdAt);
    expect(times).toEqual([...times].sort((a, b) => b - a));
    expect(new Set(bs.map((b) => b.kind))).toEqual(new Set(['world', 'snapshot']));
  });

  it('delete with back-up-first keeps a before_delete backup; without it none is made', async () => {
    const { id } = await setup();
    const ws = await loadWorlds(id);
    await loadBackups(id);
    const n = wb.backups[id]!.length;
    const made = await deleteWorld(id, ws[0], true);
    expect(made?.reason).toBe('before_delete');
    expect(wb.worlds[id]).toHaveLength(3);
    expect(wb.backups[id]).toHaveLength(n + 1);
    expect(await deleteWorld(id, ws[1], false)).toBeNull();
    expect(wb.worlds[id]).toHaveLength(2);
    expect(wb.backups[id]).toHaveLength(n + 1);
    expect(wb.busy[id]).toBeUndefined();
  });

  it('restore as new copy keeps the original; default restore of a deleted world brings it back under its name', async () => {
    const { id } = await setup();
    const ws = await loadWorlds(id);
    const bs = await loadBackups(id);
    const wbk = bs.find((b) => b.kind === 'world' && b.worldId === 'Iron Hollow')!;
    await restoreBackup(id, wbk, 'asNew');
    expect(wb.worlds[id]!.map((w) => w.id)).toContain('Iron Hollow (restored)');
    expect(wb.worlds[id]).toHaveLength(ws.length + 1);
    await deleteWorld(id, ws[1], false);
    const creative = bs.find((b) => b.worldId === 'Creative Plot')!;
    await restoreBackup(id, creative, 'default');
    expect(wb.worlds[id]!.filter((w) => w.id === 'Creative Plot')).toHaveLength(1);
  });

  it('snapshot restore takes an automatic safety snapshot first', async () => {
    const { id } = await setup();
    const bs = await loadBackups(id);
    const snap = bs.find((b) => b.kind === 'snapshot' && b.reason === 'manual')!;
    await restoreBackup(id, snap, 'default');
    expect(wb.backups[id]!.some((b) => b.kind === 'snapshot' && b.reason === 'auto' && b.label === 'Before restore')).toBe(true);
  });

  it('creates snapshots and world backups with labels and deletes backups', async () => {
    const { id } = await setup();
    const ws = await loadWorlds(id);
    await loadBackups(id);
    const s = await createSnapshot(id, '  my label ');
    expect(s).toMatchObject({ kind: 'snapshot', reason: 'manual', label: 'my label' });
    const w = await backupWorld(id, ws[0], '');
    expect(w).toMatchObject({ kind: 'world', worldId: ws[0].id });
    expect(wb.backups[id]![0].id).toBe(w.id); // newest first
    await deleteBackup(id, s);
    expect(wb.backups[id]!.some((b) => b.id === s.id)).toBe(false);
  });

  it('refuses a second job while one runs', async () => {
    const { id } = await setup({ timeScale: 1 });
    const first = createSnapshot(id, '');
    expect(wb.busy[id]?.kind).toBe('snapshot');
    await expect(createSnapshot(id, '')).rejects.toMatchObject({ code: 'backup_busy' });
    await first;
    expect(wb.busy[id]).toBeUndefined();
  }, 10000);

  it('?mockRunning makes delete and restore fail with game_running, backups still work', async () => {
    const { id } = await setup({ running: true });
    const ws = await loadWorlds(id);
    const bs = await loadBackups(id);
    await expect(deleteWorld(id, ws[0], true)).rejects.toMatchObject({ code: 'game_running' });
    await expect(restoreBackup(id, bs[0], 'default')).rejects.toMatchObject({ code: 'game_running' });
    expect(wb.worlds[id]).toHaveLength(4);
    expect(wb.busy[id]).toBeUndefined();
    await expect(backupWorld(id, ws[0], '')).resolves.toBeTruthy();
  });

  it('a stale-list error refreshes the lists', async () => {
    const { api, id } = await setup();
    const ws = await loadWorlds(id);
    await loadBackups(id);
    await api.worldDelete({ profileId: id, worldId: ws[0].id, makeBackup: false }); // deleted behind the UI's back
    await expect(backupWorld(id, ws[0], '')).rejects.toMatchObject({ code: 'world_not_found' });
    expect(wb.worlds[id]).toHaveLength(3);
  });

  it('error variants come from ?mockBackupError', async () => {
    const { id } = await setup({ backupError: 'backup_too_large' });
    const ws = await loadWorlds(id);
    await expect(backupWorld(id, ws[0], '')).rejects.toMatchObject({ code: 'backup_too_large' });
    await expect(createSnapshot(id, '')).rejects.toMatchObject({ code: 'backup_too_large' });
  });

  it('settings validation: backupKeep outside 1..50 is rejected like the backend', async () => {
    const { api } = await setup();
    await expect(api.configSet({ patch: { backupKeep: 0 } })).rejects.toMatchObject({ code: 'validation' });
    await expect(api.configSet({ patch: { backupKeep: 51 } })).rejects.toMatchObject({ code: 'validation' });
    expect((await api.configSet({ patch: { backupKeep: 12 } })).backupKeep).toBe(12);
  });

  it('prunes automatic backups beyond backupKeep but never manual ones', async () => {
    const { api, id } = await setup();
    await api.configSet({ patch: { backupKeep: 1 } });
    const ws = await loadWorlds(id);
    await deleteWorld(id, ws[0], true);
    await deleteWorld(id, ws[1], true);
    const bs = await loadBackups(id);
    expect(bs.filter((b) => b.reason !== 'manual')).toHaveLength(1);
    expect(bs.filter((b) => b.reason === 'manual').length).toBeGreaterThanOrEqual(2);
  });
});
