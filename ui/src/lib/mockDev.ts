// Dev controls of the browser mock for looking at skeleton, timeout, error and retry states (the real backends never see these).
//   ?mockLatency=1500                every command answers after 1.5 s (default: 40 ms, below the 150 ms skeleton delay)
//   ?mockDelay=searchProjects:4000,statsSummary:800   per-command latency (UI method name or Rust command name)
//   ?mockSlow=searchProjects[,..]    the FIRST call never answers (the load times out, Retry then works); `name*` = every call hangs
//   ?mockFail=statsSummary[,..]      the FIRST call fails with net.request (Retry then works); `name*` = every call fails
//   ?loadTimeout=3000                shortens the 12 s load timeout so the timeout -> error -> retry flow is quick to see
import { COMMANDS } from './apiFacade';

export interface MockDevOptions {
  base: number;
  perCommand: Record<string, number>;
  slow: { name: string; always: boolean }[];
  fail: { name: string; always: boolean }[];
}

export const DEFAULT_MOCK_LATENCY_MS = 40;

const toMethod = (name: string): string => {
  const hit = Object.entries(COMMANDS).find(([, cmd]) => cmd === name);
  return hit ? hit[0] : name;
};
const list = (v: string | null) =>
  (v ?? '').split(',').map((s) => s.trim()).filter(Boolean).map((s) => ({ name: toMethod(s.replace(/\*$/, '')), always: s.endsWith('*') }));

export function mockDevFromSearch(search: string): MockDevOptions {
  const q = new URLSearchParams(search);
  const n = Number(q.get('mockLatency'));
  const perCommand: Record<string, number> = {};
  for (const part of (q.get('mockDelay') ?? '').split(',')) {
    const [name, ms] = part.split(':');
    if (name && Number.isFinite(Number(ms))) perCommand[toMethod(name.trim())] = Number(ms);
  }
  return {
    base: q.has('mockLatency') && Number.isFinite(n) && n >= 0 ? n : DEFAULT_MOCK_LATENCY_MS,
    perCommand, slow: list(q.get('mockSlow')), fail: list(q.get('mockFail')),
  };
}

/** Wraps every command of a mock api (not `listen`, not `dev`) with the configured latency / hang / failure. */
export function withDevControls<T extends object>(api: T, o: MockDevOptions): T {
  const used = new Set<string>();
  const out: Record<string, unknown> = { ...(api as Record<string, unknown>) };
  for (const [key, fn] of Object.entries(api as Record<string, unknown>)) {
    if (typeof fn !== 'function' || key === 'listen') continue;
    out[key] = async (...args: unknown[]) => {
      const slow = o.slow.find((s) => s.name === key);
      if (slow && (slow.always || !used.has(`slow:${key}`))) { used.add(`slow:${key}`); await new Promise<never>(() => {}); }
      const ms = o.perCommand[key] ?? o.base;
      if (ms > 0) await new Promise((r) => setTimeout(r, ms));
      const fail = o.fail.find((s) => s.name === key);
      if (fail && (fail.always || !used.has(`fail:${key}`))) {
        used.add(`fail:${key}`);
        throw { code: 'net.request', message: 'mock failure', needsRelogin: false };
      }
      return (fn as (...a: unknown[]) => unknown).apply(api, args);
    };
  }
  return out as T;
}
