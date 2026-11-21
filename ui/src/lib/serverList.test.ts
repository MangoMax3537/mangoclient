import { describe, expect, it } from 'vitest';
import { arrangeServers, joinPlan, joinRows, serverFacts } from './serverList';
import type { ServerEntry, ServerStatus } from './types';

const e = (id: string, name: string, o: Partial<ServerEntry> = {}): ServerEntry => ({ id, name, address: `${id}.example.com`, partner: false, starred: false, custom: false, iconUrl: null, ...o });
const st = (id: string, ping: number | null, online_: number, range: { min: string; max: string } | null = null): ServerStatus => ({
  id, online: ping !== null, ping, motd: '', players: ping === null ? null : { online: online_, max: 100 }, version: null, versionRange: range, iconUrl: null,
});
const list = [e('p', 'VincentVanilla', { partner: true, starred: true }), e('a', 'Zeta'), e('b', 'alpha', { starred: true }), e('c', 'Mid'), e('d', 'Beta', { starred: true })];
const status = { p: st('p', 90, 5), a: st('a', 20, 50), b: st('b', 60, 10), c: st('c', null, 0), d: st('d', 30, 99) };
const ids = (l: ServerEntry[]) => l.map((s) => s.id);

describe('server order', () => {
  it('the partner is always first, then favourites, then the rest; manual keeps the list order inside a group', () => {
    expect(ids(arrangeServers(list, status, 'manual'))).toEqual(['p', 'b', 'd', 'a', 'c']);
  });
  it('sorts inside the groups by name, ping (offline last) and players (offline last)', () => {
    expect(ids(arrangeServers(list, status, 'name'))).toEqual(['p', 'b', 'd', 'c', 'a']);
    expect(ids(arrangeServers(list, status, 'ping'))).toEqual(['p', 'd', 'b', 'a', 'c']);
    expect(ids(arrangeServers(list, status, 'players'))).toEqual(['p', 'd', 'b', 'a', 'c']);
  });
  it('the partner stays first even when it is the slowest and unstarred in the data', () => {
    const l = [e('x', 'X', { starred: true }), e('p', 'P', { partner: true })];
    for (const s of ['manual', 'ping', 'players', 'name'] as const) expect(ids(arrangeServers(l, { x: st('x', 1, 1), p: st('p', 900, 0) }, s))[0]).toBe('p');
  });
  it('search matches name and address, never hides the partner', () => {
    expect(ids(arrangeServers(list, status, 'manual', 'ALP'))).toEqual(['p', 'b']);
    expect(ids(arrangeServers(list, status, 'manual', 'c.example'))).toEqual(['p', 'c']);
    expect(ids(arrangeServers(list, status, 'manual', 'nothing'))).toEqual(['p']);
  });
  it('does not mutate its input', () => {
    const copy = [...list];
    arrangeServers(list, status, 'name');
    expect(list).toEqual(copy);
  });
});

describe('which instance joins', () => {
  const profiles = [{ id: 'old', mcVersion: '1.8.9' }, { id: 'new', mcVersion: '1.21.11' }, { id: 'snap', mcVersion: '24w14a' }, { id: 'run', mcVersion: '1.21.8' }];
  const s = st('s', 10, 1, { min: '1.21.5', max: '26.x' });
  const busy = (id: string) => id === 'run';
  it('compatible first (selected on top), then not sure, then disabled with the reason', () => {
    const rows = joinRows(profiles, s, 'new', busy);
    expect(rows.map((r) => r.id)).toEqual(['new', 'snap', 'old', 'run']);
    expect(rows.map((r) => r.enabled)).toEqual([true, true, false, false]);
    expect(rows.find((r) => r.id === 'old')?.reason).toBe('incompatible');
    expect(rows.find((r) => r.id === 'run')?.reason).toBe('busy');
    expect(rows.find((r) => r.id === 'snap')?.compat).toBe('unknown');
  });
  it('a selected compatible instance on top regardless of list order', () => {
    expect(joinRows([{ id: 'x', mcVersion: '1.21.6' }, { id: 'y', mcVersion: '1.21.7' }], s, 'y', () => false).map((r) => r.id)).toEqual(['y', 'x']);
  });
  it('joins directly only when the selected instance is known to fit and is not running', () => {
    expect(joinPlan(profiles, s, 'new', busy)).toEqual({ kind: 'direct', profileId: 'new' });
    expect(joinPlan(profiles, s, 'old', busy)).toEqual({ kind: 'choose' });
    expect(joinPlan(profiles, s, 'run', busy)).toEqual({ kind: 'choose' });
    expect(joinPlan(profiles, s, 'snap', busy)).toEqual({ kind: 'choose' });
    expect(joinPlan(profiles, st('s', 10, 1, null), 'new', busy)).toEqual({ kind: 'choose' });
    expect(joinPlan(profiles, undefined, 'new', busy)).toEqual({ kind: 'choose' });
    expect(joinPlan([], s, null, busy)).toEqual({ kind: 'choose' });
  });
});

describe('server facts', () => {
  const tr = ((k: string, v?: { n?: number }) => (k === 'servers.ms' ? `${v?.n} ms` : 'Offline')) as never;
  it('range, players and ping only; offline and unknown cases', () => {
    expect(serverFacts(st('s', 10, 0, { min: '1.21.11', max: '26.3' }), tr)).toEqual({
      version: [{ kind: 'version', text: '1.21.11 - 26.3' }],
      stats: [{ kind: 'players', text: '0/100' }, { kind: 'ping', text: '10 ms' }],
    });
    expect(serverFacts(st('s', 10, 3), tr).version[0].text).toBe('–');
    expect(serverFacts(st('s', null, 0), tr).stats).toEqual([{ kind: 'ping', text: 'Offline', dim: true }]);
    expect(serverFacts(undefined, tr)).toEqual({ version: [{ kind: 'version', text: '–' }], stats: [] });
  });
});
