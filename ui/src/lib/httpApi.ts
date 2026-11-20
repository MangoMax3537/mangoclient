// Browser adapter for the Rust dev server (`mango-dev`, crates/devserver): the real service core behind fetch + Server-Sent Events,
// so the UI can be driven in a plain browser. Selected by `?backend=http://127.0.0.1:<port>&token=<token>`. Development only.
// The token lives in memory (a closure) only: it is never stored, never logged and never part of an error message.
import { createApi, type Api, type Invoke, type Subscribe } from './apiFacade';
import type { AppError, CommandName, EventName } from './generated';

export interface BackendParams { base: string; token: string }

/** Reads `?backend=&token=`. Only plain-http loopback origins are accepted (the dev server binds 127.0.0.1 only). */
export function backendFromSearch(search: string): BackendParams | null {
  const q = new URLSearchParams(search);
  const raw = q.get('backend');
  const token = q.get('token');
  if (!raw || !token) return null;
  try {
    const u = new URL(raw);
    const loopback = u.hostname === '127.0.0.1' || u.hostname === 'localhost' || u.hostname === '[::1]';
    if (u.protocol !== 'http:' || !loopback || u.username || u.password) return null;
    return { base: u.origin, token };
  } catch { return null; }
}

const IMG = 'mangoimg://';

/** `mangoimg://k/r` -> `${base}/img/k/r?token=...`. Applied to every string of every result and event. */
export function rewriteImages<T>(value: T, base: string, token: string): T {
  const walk = (v: unknown): unknown => {
    if (typeof v === 'string') return v.startsWith(IMG) ? `${base}/img/${v.slice(IMG.length)}?token=${encodeURIComponent(token)}` : v;
    if (Array.isArray(v)) return v.map(walk);
    if (v && typeof v === 'object') return Object.fromEntries(Object.entries(v).map(([k, x]) => [k, walk(x)]));
    return v;
  };
  return walk(value) as T;
}

const isAppError = (e: unknown): e is AppError =>
  !!e && typeof e === 'object' && typeof (e as AppError).code === 'string' && typeof (e as AppError).message === 'string';

export interface HttpOptions {
  fetch?: typeof fetch;
  EventSource?: typeof EventSource;
}

export function createHttpApi({ base, token }: BackendParams, opts: HttpOptions = {}): Api {
  const doFetch = opts.fetch ?? ((...a: Parameters<typeof fetch>) => fetch(...a));
  const ES = opts.EventSource ?? (typeof EventSource !== 'undefined' ? EventSource : undefined);

  const invoke: Invoke = async (command: CommandName, args) => {
    let res: Response;
    try {
      res = await doFetch(`${base}/invoke/${command}`, {
        method: 'POST',
        headers: { Authorization: `Bearer ${token}`, 'Content-Type': 'application/json' },
        body: JSON.stringify(args ?? {}),
      });
    } catch {
      throw { code: 'backend_unreachable', message: `dev server at ${base} is not reachable`, needsRelogin: false } satisfies AppError;
    }
    let body: unknown = null;
    const text = await res.text();
    if (text) { try { body = JSON.parse(text); } catch { body = null; } }
    if (!res.ok) {
      throw isAppError(body) ? body : { code: 'http_error', message: `dev server answered ${res.status}`, needsRelogin: false } satisfies AppError;
    }
    return rewriteImages(body, base, token) as never;
  };

  // One shared EventSource for all subscriptions, opened with the first one and closed with the last.
  const subs = new Map<string, Set<(p: never) => void>>();
  let source: EventSource | null = null;
  const open = () => {
    if (source || !ES) return;
    source = new ES(`${base}/events?token=${encodeURIComponent(token)}`);
    source.onmessage = (m: MessageEvent<string>) => {
      let msg: { name?: string; payload?: unknown };
      try { msg = JSON.parse(m.data); } catch { return; }
      const set = msg.name ? subs.get(msg.name) : undefined;
      if (!set?.size) return;
      const payload = rewriteImages(msg.payload, base, token) as never;
      for (const cb of [...set]) cb(payload);
    };
  };
  const listen: Subscribe = async (name: EventName, cb) => {
    const set = subs.get(name) ?? new Set();
    subs.set(name, set);
    set.add(cb as (p: never) => void);
    open();
    return () => {
      set.delete(cb as (p: never) => void);
      if (![...subs.values()].some((s) => s.size) && source) { source.close(); source = null; }
    };
  };

  return createApi({ invoke, listen });
}
