import { describe, expect, it, vi } from 'vitest';
import { backendFromSearch, createHttpApi, rewriteImages } from './httpApi';
import { backendKind } from './api';

const P = { base: 'http://127.0.0.1:4567', token: 'tok/en+1' };

function res(status: number, body: unknown): Response {
  return new Response(body === undefined ? '' : JSON.stringify(body), { status, headers: { 'Content-Type': 'application/json' } });
}

class FakeES {
  static last: FakeES | null = null;
  onmessage: ((m: MessageEvent<string>) => void) | null = null;
  closed = false;
  constructor(public url: string) { FakeES.last = this; }
  close() { this.closed = true; }
  push(data: unknown) { this.onmessage?.({ data: JSON.stringify(data) } as MessageEvent<string>); }
}

describe('backend selection', () => {
  it('accepts only plain-http loopback origins with a token', () => {
    expect(backendFromSearch('?backend=http://127.0.0.1:9000&token=abc')).toEqual({ base: 'http://127.0.0.1:9000', token: 'abc' });
    expect(backendFromSearch('?backend=http://localhost:9000/x?y&token=abc')?.base).toBe('http://localhost:9000');
    expect(backendFromSearch('?backend=http://127.0.0.1:9000')).toBeNull();
    expect(backendFromSearch('?token=abc')).toBeNull();
    expect(backendFromSearch('?backend=https://127.0.0.1:9000&token=abc')).toBeNull();
    expect(backendFromSearch('?backend=http://evil.example:9000&token=abc')).toBeNull();
    expect(backendFromSearch('?backend=http://u:p@127.0.0.1:9000&token=abc')).toBeNull();
    expect(backendFromSearch('?backend=nonsense&token=abc')).toBeNull();
  });
  it('Tauri wins, then http, then the mock', () => {
    expect(backendKind('?backend=http://127.0.0.1:1&token=t', true)).toBe('tauri');
    expect(backendKind('?backend=http://127.0.0.1:1&token=t', false)).toBe('http');
    expect(backendKind('', false)).toBe('mock');
    expect(backendKind('?backend=http://example.com&token=t', false)).toBe('mock');
  });
});

describe('mangoimg rewriting', () => {
  it('maps every mangoimg:// string, deep, and nothing else', () => {
    const out = rewriteImages({ a: 'mangoimg://head/u1', n: [{ u: 'mangoimg://block/dirt/top' }], keep: 'https://x/y', num: 3, nil: null }, P.base, P.token);
    expect(out).toEqual({
      a: `${P.base}/img/head/u1?token=tok%2Fen%2B1`, n: [{ u: `${P.base}/img/block/dirt/top?token=tok%2Fen%2B1` }],
      keep: 'https://x/y', num: 3, nil: null,
    });
  });
});

describe('http invoke', () => {
  it('POSTs camelCase JSON with the bearer token and rewrites images in the result', async () => {
    const f = vi.fn(async () => res(200, { uuid: 'u', headUrl: 'mangoimg://head/u' }));
    const api = createHttpApi(P, { fetch: f as unknown as typeof fetch });
    const a = await api.accountAddOffline({ name: 'Steve' });
    expect(a.headUrl).toBe(`${P.base}/img/head/u?token=tok%2Fen%2B1`);
    const [url, init] = f.mock.calls[0] as unknown as [string, RequestInit];
    expect(url).toBe(`${P.base}/invoke/account_add_offline`);
    expect(init.method).toBe('POST');
    expect((init.headers as Record<string, string>).Authorization).toBe(`Bearer ${P.token}`);
    expect(init.body).toBe('{"name":"Steve"}');
    expect(url).not.toContain('tok'); // the token is only in the header for invoke
  });

  it('argument-less commands send {} and a null body resolves to null', async () => {
    const f = vi.fn(async () => res(200, null));
    const api = createHttpApi(P, { fetch: f as unknown as typeof fetch });
    expect(await api.serversPingAll()).toBeNull();
    expect((f.mock.calls[0] as unknown as [string, RequestInit])[1].body).toBe('{}');
  });

  it('throws the AppError body of a failed command unchanged', async () => {
    const err = { code: 'background.bad_style', message: 'dim out of range', needsRelogin: false };
    const api = createHttpApi(P, { fetch: (async () => res(400, err)) as unknown as typeof fetch });
    await expect(api.profileBackgroundSetStyle({ id: 'p', dim: 2 })).rejects.toEqual(err);
  });

  it('maps an unreadable error body and a network failure to stable codes without leaking the token', async () => {
    const api1 = createHttpApi(P, { fetch: (async () => new Response('<html>', { status: 502 })) as unknown as typeof fetch });
    await expect(api1.appState()).rejects.toMatchObject({ code: 'http_error' });
    const api2 = createHttpApi(P, { fetch: (async () => { throw new TypeError(`failed ${P.token}`); }) as unknown as typeof fetch });
    const e = await api2.appState().catch((x) => x);
    expect(e.code).toBe('backend_unreachable');
    expect(JSON.stringify(e)).not.toContain('tok');
  });
});

describe('http events', () => {
  it('shares one EventSource, dispatches by name with rewritten payloads, closes with the last listener', async () => {
    const api = createHttpApi(P, { fetch: vi.fn() as unknown as typeof fetch, EventSource: FakeES as unknown as typeof EventSource });
    const states: unknown[] = [];
    const blocks: unknown[] = [];
    const off1 = await api.listen('launch:state', (p) => states.push(p));
    const off2 = await api.listen('blocks:status', (p) => blocks.push(p));
    const es = FakeES.last as FakeES;
    expect(es.url).toBe(`${P.base}/events?token=tok%2Fen%2B1`);
    es.push({ name: 'launch:state', payload: { profileId: 'p', state: 'running' } });
    es.push({ name: 'blocks:status', payload: { state: 'ready', mcVersion: null, blocks: [{ id: 'dirt', label: 'Dirt', faces: { top: 'mangoimg://block/dirt/top', left: 'a', right: 'b' } }] } });
    es.push({ name: 'unknown:event', payload: {} });
    es.onmessage?.({ data: 'not json' } as MessageEvent<string>);
    expect(states).toEqual([{ profileId: 'p', state: 'running' }]);
    expect((blocks[0] as { blocks: { faces: { top: string } }[] }).blocks[0].faces.top).toContain('/img/block/dirt/top?token=');
    off1();
    expect(es.closed).toBe(false);
    off2();
    expect(es.closed).toBe(true);
  });
});
