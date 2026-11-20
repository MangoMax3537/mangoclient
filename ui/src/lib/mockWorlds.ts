// Mock of the Part 9 world / backup / snapshot commands: realistic seeded data, state changes, a game_running simulation and error variants.
import type { Api } from './api';
import type { Backup, BackupReason, Config, ProfileView, World } from './types';

export interface WorldsMockDeps {
  scale: number;
  now: number;
  config: () => Config;
  profiles: () => ProfileView[];
  find: (id: string) => ProfileView;
  /** The game of this instance is running (or starting). */
  gameActive: (id: string) => boolean;
  /** `?mockBackupError=<code>`: every changing call fails with this code. */
  errorCode?: string;
}

const DAY = 86400000;
const MB = 1048576;
const err = (code: string, message = code) => ({ code, message, needsRelogin: false });
const safe = (s: string) => s.toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-|-$/g, '').slice(0, 40);

function seedWorlds(profile: ProfileView, index: number, now: number): World[] {
  const w = (id: string, name: string, p: Partial<World>): World => ({
    id, name, lastPlayed: null, sizeBytes: 40 * MB, gameMode: 'survival', hardcore: false, version: profile.mcVersion, iconUrl: null, ...p,
  });
  if (index === 0) {
    return [
      w('Iron Hollow', 'Iron Hollow', { lastPlayed: now - 2 * 3600000, sizeBytes: 612 * MB, version: profile.mcVersion }),
      w('Creative Plot', 'Creative Plot', { lastPlayed: now - 3 * DAY, sizeBytes: 148 * MB, gameMode: 'creative' }),
      w('Hardcore Run #3', 'Hardcore Run #3', { lastPlayed: now - 9 * DAY, sizeBytes: 87 * MB, hardcore: true }),
      w('Old Castle', 'Old Castle (1.12)', { lastPlayed: now - 410 * DAY, sizeBytes: 23 * MB, version: '1.12.2', gameMode: 'adventure' }),
    ];
  }
  if (index === 1) {
    return [
      w('New World', 'New World', { lastPlayed: now - 1 * DAY, sizeBytes: 301 * MB }),
      w('Spectate Test', 'Spectate Test', { lastPlayed: null, sizeBytes: 5 * MB, gameMode: 'spectator', version: null }),
    ];
  }
  return [];
}

function seedBackups(profile: ProfileView, now: number): Backup[] {
  const b = (ageMs: number, kind: Backup['kind'], reason: BackupReason, label: string, size: number, worldId: string | null): Backup => ({
    id: `${now - ageMs}-${kind}-${safe(label || worldId || 'snapshot')}`, kind, profileId: profile.id, worldId, createdAt: now - ageMs, sizeBytes: size * MB, label, reason,
  });
  return [
    b(3 * 3600000, 'snapshot', 'auto', 'Before update', 42, null),
    b(1 * DAY, 'world', 'manual', 'Before the Nether portal', 598, 'Iron Hollow'),
    b(2 * DAY, 'snapshot', 'before_update', 'Update all', 40, null),
    b(8 * DAY, 'world', 'before_delete', 'Hardcore Run #2', 74, 'Hardcore Run #2'),
    b(21 * DAY, 'snapshot', 'manual', 'Working shader setup', 39, null),
    b(35 * DAY, 'world', 'auto', '', 140, 'Creative Plot'),
  ];
}

export type WorldsMock = Pick<Api, 'worldsList' | 'worldOpenFolder' | 'worldBackup' | 'worldDelete' | 'backupsList' | 'backupRestore' | 'backupDelete' | 'snapshotCreate'>;

export function createWorldsMock(d: WorldsMockDeps): WorldsMock {
  const worlds = new Map<string, World[]>();
  const backups = new Map<string, Backup[]>();
  const ensure = (id: string) => {
    const p = d.find(id);
    if (!worlds.has(id)) {
      const i = d.profiles().findIndex((x) => x.id === id);
      worlds.set(id, seedWorlds(p, i, d.now));
      backups.set(id, i === 0 ? seedBackups(p, d.now) : []);
    }
    return { w: worlds.get(id) as World[], b: backups.get(id) as Backup[] };
  };
  const sleep = (ms: number) => new Promise<void>((r) => setTimeout(r, ms * d.scale));
  const sorted = (l: Backup[]) => [...l].sort((a, b) => b.createdAt - a.createdAt);
  const forced = () => { if (d.errorCode) throw err(d.errorCode); };
  const blockedByGame = (id: string) => { if (d.gameActive(id)) throw err('game_running', 'Close the game before changing this instance'); };
  /** Automatic backups beyond `backupKeep` are pruned, manual ones never. */
  const prune = (id: string) => {
    const keep = d.config().backupKeep;
    const autos = sorted(backups.get(id) ?? []).filter((x) => x.reason !== 'manual');
    const drop = new Set(autos.slice(keep).map((x) => x.id));
    backups.set(id, (backups.get(id) ?? []).filter((x) => !drop.has(x.id)));
  };
  let seq = 0;
  const add = (id: string, kind: Backup['kind'], reason: BackupReason, label: string, size: number, worldId: string | null): Backup => {
    const createdAt = Date.now() + seq++;
    const b: Backup = { id: `${createdAt}-${kind}-${safe(label || worldId || 'snapshot')}`, kind, profileId: id, worldId, createdAt, sizeBytes: size, label, reason };
    backups.get(id)?.push(b);
    prune(id);
    return b;
  };

  return {
    async worldsList({ profileId }) { return structuredClone(ensure(profileId).w); },
    async worldOpenFolder({ profileId }) { ensure(profileId); return `~/.mangoclient/instances/${profileId}/saves`; },
    async backupsList({ profileId }) { return structuredClone(sorted(ensure(profileId).b)); },
    async worldBackup({ profileId, worldId, label }) {
      const { w } = ensure(profileId);
      forced();
      const world = w.find((x) => x.id === worldId);
      if (!world) throw err('world_not_found', 'World not found');
      await sleep(1400);
      return structuredClone(add(profileId, 'world', 'manual', label?.trim() ?? '', Math.round(world.sizeBytes * 0.62), world.id));
    },
    async worldDelete({ profileId, worldId, makeBackup }) {
      const s = ensure(profileId);
      forced(); blockedByGame(profileId);
      const world = s.w.find((x) => x.id === worldId);
      if (!world) throw err('world_not_found', 'World not found');
      let made: Backup | null = null;
      if (makeBackup) { await sleep(1200); made = add(profileId, 'world', 'before_delete', world.name, Math.round(world.sizeBytes * 0.62), world.id); }
      worlds.set(profileId, s.w.filter((x) => x.id !== worldId));
      return made ? structuredClone(made) : null;
    },
    async snapshotCreate({ profileId, label }) {
      const p = ensure(profileId);
      forced();
      await sleep(1600);
      return structuredClone(add(profileId, 'snapshot', 'manual', label?.trim() ?? '', Math.max(2, p.w.length ? 38 * MB : 4 * MB), null));
    },
    async backupRestore({ profileId, backupId, asNew }) {
      const s = ensure(profileId);
      forced(); blockedByGame(profileId);
      const b = s.b.find((x) => x.id === backupId);
      if (!b) throw err('backup_not_found', 'Backup not found');
      await sleep(1500);
      if (b.kind === 'snapshot') {
        add(profileId, 'snapshot', 'auto', 'Before restore', b.sizeBytes, null); // the automatic safety snapshot of the current state
      } else {
        const id = b.worldId ?? safe(b.label);
        const taken = new Set(s.w.map((x) => x.id));
        let folder = id; let n = 2;
        if (asNew || taken.has(id)) { folder = `${id} (restored)`; while (taken.has(folder)) folder = `${id} (restored ${n++})`; }
        s.w.push({ id: folder, name: folder, lastPlayed: b.createdAt, sizeBytes: Math.round(b.sizeBytes * 1.6), gameMode: 'survival', hardcore: false, version: d.find(profileId).mcVersion, iconUrl: null });
      }
      return structuredClone(sorted(backups.get(profileId) ?? []));
    },
    async backupDelete({ profileId, backupId }) {
      const s = ensure(profileId);
      forced();
      if (!s.b.some((x) => x.id === backupId)) throw err('backup_not_found', 'Backup not found');
      await sleep(250);
      backups.set(profileId, s.b.filter((x) => x.id !== backupId));
      return structuredClone(sorted(backups.get(profileId) ?? []));
    },
  };
}
