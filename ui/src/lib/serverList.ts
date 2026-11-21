// Pure helpers of the server views: sorting, searching and which instance can join a server.
import type { Fact } from './infoLine';
import { compatibility, rangeLabel, type Compat } from './serverVersion';
import type { ServerEntry, ServerStatus } from './types';

export type ServerSort = 'manual' | 'ping' | 'players' | 'name';
export const SERVER_SORTS: readonly ServerSort[] = ['manual', 'ping', 'players', 'name'];

const online = (s?: ServerStatus): boolean => !!s?.online;
const byName = (a: ServerEntry, b: ServerEntry): number => a.name.localeCompare(b.name, undefined, { sensitivity: 'base' });
const rank: Record<ServerSort, (a: ServerEntry, b: ServerEntry, st: Record<string, ServerStatus>) => number> = {
  manual: () => 0,
  name: byName,
  // offline and not yet pinged servers sink to the end; ties fall back to the name
  ping: (a, b, st) => {
    const x = online(st[a.id]) ? st[a.id].ping ?? Infinity : Infinity, y = online(st[b.id]) ? st[b.id].ping ?? Infinity : Infinity;
    return x === y ? byName(a, b) : x - y;
  },
  players: (a, b, st) => {
    const x = online(st[a.id]) ? st[a.id].players?.online ?? -1 : -1, y = online(st[b.id]) ? st[b.id].players?.online ?? -1 : -1;
    return x === y ? byName(a, b) : y - x;
  },
};

/**
 * The displayed order. Partner servers are always first and keep their list order, whatever the sort or the stars; then the starred ones;
 * then the rest. The sort applies inside the starred and the plain group. `query` filters name and address (not the partners).
 */
export function arrangeServers(list: readonly ServerEntry[], status: Record<string, ServerStatus>, sort: ServerSort, query = ''): ServerEntry[] {
  const q = query.trim().toLowerCase();
  const match = (s: ServerEntry) => s.partner || !q || s.name.toLowerCase().includes(q) || s.address.toLowerCase().includes(q);
  const kept = list.filter(match);
  const group = (pred: (s: ServerEntry) => boolean) => kept.filter(pred).sort((a, b) => rank[sort](a, b, status));
  return [...kept.filter((s) => s.partner), ...group((s) => !s.partner && s.starred), ...group((s) => !s.partner && !s.starred)];
}

export interface JoinRow {
  id: string;
  compat: Compat;
  /** Can be chosen. Incompatible and busy instances are disabled, "not sure" ones are not. */
  enabled: boolean;
  reason: 'incompatible' | 'busy' | null;
}
interface P { id: string; mcVersion: string }

/** One row per instance: compatible first (the selected one at the top), then "not sure", then the disabled ones; stable inside a class. */
export function joinRows(profiles: readonly P[], status: ServerStatus | undefined, selectedId: string | null, busy: (id: string) => boolean): JoinRow[] {
  const range = status?.versionRange ?? null;
  const rows = profiles.map((p): JoinRow => {
    const compat = compatibility(range, p.mcVersion);
    const reason = busy(p.id) ? 'busy' : compat === 'no' ? 'incompatible' : null;
    return { id: p.id, compat, enabled: reason === null, reason };
  });
  const cls = (r: JoinRow) => (!r.enabled ? 3 : r.compat === 'yes' ? 0 : 1);
  return rows
    .map((r, i) => ({ r, i }))
    .sort((a, b) => cls(a.r) - cls(b.r) || Number(b.r.id === selectedId) - Number(a.r.id === selectedId) || a.i - b.i)
    .map((x) => x.r);
}

/** Clicking a server: start the selected instance at once when it is known to fit, otherwise ask. */
export function joinPlan(profiles: readonly P[], status: ServerStatus | undefined, selectedId: string | null, busy: (id: string) => boolean): { kind: 'direct'; profileId: string } | { kind: 'choose' } {
  const sel = profiles.find((p) => p.id === selectedId);
  if (sel && !busy(sel.id) && compatibility(status?.versionRange ?? null, sel.mcVersion) === 'yes') return { kind: 'direct', profileId: sel.id };
  return { kind: 'choose' };
}

type Tr = (k: 'servers.ms' | 'servers.offline', v?: Record<string, string | number>) => string;
/** The info lines of a server: supported versions; players and ping. Only facts, no description. Before the first ping: nothing but a dash for the version. */
export function serverFacts(st: ServerStatus | undefined, t: Tr): { version: Fact[]; stats: Fact[] } {
  const label = rangeLabel(st?.versionRange);
  const version: Fact[] = [{ kind: 'version', text: label ?? '–' }];
  if (!st) return { version, stats: [] };
  if (!st.online) return { version, stats: [{ kind: 'ping', text: t('servers.offline'), dim: true }] };
  const stats: Fact[] = [];
  if (st.players) stats.push({ kind: 'players', text: `${st.players.online}/${st.players.max}` });
  if (st.ping !== null) stats.push({ kind: 'ping', text: t('servers.ms', { n: st.ping }) });
  return { version, stats };
}
