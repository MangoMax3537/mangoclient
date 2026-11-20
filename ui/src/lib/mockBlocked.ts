// Part 5 of the mock backend: blocked CurseForge files, the folder watcher and a dev helper that plays the user.
import type { Api } from './api';
import type { ApiEvents, BlockedFile } from './types';

export type PartBlocked = Pick<Api, 'blockedFiles' | 'blockedWatchStart' | 'blockedWatchStop' | 'blockedSkip' | 'blockedOpenFolder'>;

/** What a developer (or a test) can do next to the app: put a file into the Downloads folder as the user would. */
export interface BlockedDev {
  /** Downloads one pending file (the named one, else the next; without a profile id: the newest instance with blocked files); resolves its file name or null. The watcher picks it up on its next tick. */
  download(profileId?: string, fileName?: string): string | null;
  /** Whether a watcher is running for the profile. */
  watching(profileId: string): boolean;
}

export interface BlockedMockCtx {
  scale: number;
  emit: <K extends keyof ApiEvents>(name: K, payload: ApiEvents[K]) => void;
  /** Config blockedModsWatchDirs. */
  dirs: () => string[];
  setPending: (profileId: string, n: number) => void;
  /** Every pending file shows up in Downloads by itself, one per tick (?mockBlockedAuto). */
  auto: boolean;
}

const pendingCount = (files: BlockedFile[]) => files.filter((f) => f.state === 'pending' || f.state === 'found').length;

export function createBlockedMock(ctx: BlockedMockCtx): { api: PartBlocked; seed: (profileId: string, files: BlockedFile[]) => number; dev: BlockedDev } {
  const store = new Map<string, BlockedFile[]>();
  const downloaded = new Map<string, Set<string>>();
  const watchers = new Map<string, ReturnType<typeof setInterval>>();
  const timers = new Set<ReturnType<typeof setTimeout>>();

  const get = (id: string) => store.get(id) ?? [];
  const publish = (id: string) => {
    const files = get(id);
    ctx.setPending(id, pendingCount(files));
    ctx.emit('blocked:changed', { profileId: id, files: structuredClone(files) });
  };
  const dl = (id: string) => downloaded.get(id) ?? downloaded.set(id, new Set()).get(id)!;

  function tick(id: string) {
    const next = get(id).find((f) => f.state === 'pending' && (ctx.auto || dl(id).has(f.fileName)));
    if (!next) return;
    next.state = 'found';
    next.foundPath = `${ctx.dirs()[0] ?? '/home/user/Downloads'}/${next.fileName}`;
    publish(id);
    timers.add(setTimeout(() => {
      if (next.state !== 'found') return;
      next.state = 'installed';
      publish(id);
    }, 600 * ctx.scale));
  }

  const api: PartBlocked = {
    async blockedFiles({ profileId }) { return structuredClone(get(profileId)); },
    async blockedWatchStart({ profileId }) {
      if (!watchers.has(profileId)) watchers.set(profileId, setInterval(() => tick(profileId), 1500 * ctx.scale));
      return [...ctx.dirs()];
    },
    async blockedWatchStop({ profileId }) {
      const w = watchers.get(profileId);
      if (w !== undefined) clearInterval(w);
      watchers.delete(profileId);
    },
    async blockedSkip({ profileId, fileName }) {
      const f = get(profileId).find((x) => x.fileName === fileName);
      if (!f) throw { code: 'blocked_file_not_found', message: 'no such file', needsRelogin: false };
      if (f.state === 'pending') { f.state = 'skipped'; publish(profileId); }
      return structuredClone(get(profileId));
    },
    async blockedOpenFolder() { return ctx.dirs()[0] ?? '/home/user/Downloads'; },
  };
  return {
    api,
    seed(profileId, files) { store.set(profileId, structuredClone(files)); return pendingCount(files); },
    dev: {
      download(profileIdArg, fileName) {
        const profileId = profileIdArg ?? [...store.keys()].reverse().find((id) => get(id).some((x) => x.state === 'pending')) ?? '';
        const f = get(profileId).find((x) => x.state === 'pending' && !dl(profileId).has(x.fileName) && (!fileName || x.fileName === fileName));
        if (!f) return null;
        dl(profileId).add(f.fileName);
        return f.fileName;
      },
      watching: (profileId) => watchers.has(profileId),
    },
  };
}
