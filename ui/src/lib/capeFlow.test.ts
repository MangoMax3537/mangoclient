import { describe, expect, it } from 'vitest';
import { capeError, createCapeFlow, retrySeconds, withActiveCape } from './capeFlow';
import type { SkinInfo } from './types';

const base = (): SkinInfo => ({ skinUrl: 's', capeUrl: 'u0', slim: false, source: 'account', canEdit: true, stale: false,
  capes: [{ id: 'c0', alias: 'A', url: 'u0', active: true }, { id: 'c1', alias: 'B', url: 'u1', active: false }] });

function harness(call: (id: string | null) => Promise<SkinInfo>, timeoutMs = 50) {
  let shown: SkinInfo = base(); const shownLog: (string | null)[] = []; const saving: (string | null | undefined)[] = []; const errs: string[] = [];
  const flow = createCapeFlow({
    current: () => shown, commit: (i) => { shown = i; shownLog.push(i.capes.find((c) => c.active)?.id ?? null); }, call,
    saving: (s) => saving.push(s), fail: (e) => errs.push(e.key), timeoutMs,
  });
  return { flow, get shown() { return shown; }, shownLog, saving, errs };
}

describe('withActiveCape', () => {
  it('activates a cape and moves capeUrl, null clears, unknown id is ignored', () => {
    expect(withActiveCape(base(), 'c1').capeUrl).toBe('u1');
    expect(withActiveCape(base(), null).capeUrl).toBeNull();
    expect(withActiveCape(base(), 'zz')).toEqual(base());
  });
});
describe('createCapeFlow', () => {
  it('is optimistic: the new cape is shown before the backend answers', async () => {
    let release!: (i: SkinInfo) => void;
    const h = harness(() => new Promise((r) => { release = r; }), 1000);
    const p = h.flow.request('c1');
    expect(h.shownLog).toEqual(['c1']); expect(h.saving).toEqual(['c1']); expect(h.flow.busy).toBe(true);
    release(withActiveCape(base(), 'c1'));
    expect(await p).toBe('ok'); expect(h.saving.at(-1)).toBeUndefined(); expect(h.flow.busy).toBe(false);
  });
  it('rolls back and reports on failure', async () => {
    const h = harness(() => Promise.reject({ code: 'skin.cape_not_applied', message: '', needsRelogin: false }));
    expect(await h.flow.request('c1')).toBe('rolledBack');
    expect(h.shownLog).toEqual(['c1', 'c0']); expect(h.errs).toEqual(['err.skin.cape_not_applied']);
  });
  it('times out into a rollback with a retryable error', async () => {
    const h = harness(() => new Promise(() => {}), 20);
    expect(await h.flow.request('c1')).toBe('rolledBack'); expect(h.errs).toEqual(['err.skin.timeout']);
  });
  it('ignores requests while one is in flight, and the already active cape', async () => {
    let release!: (i: SkinInfo) => void;
    const h = harness(() => new Promise((r) => { release = r; }), 1000);
    expect(await h.flow.request('c0')).toBe('noop');
    const p = h.flow.request('c1');
    expect(await h.flow.request(null)).toBe('ignored'); expect(await h.flow.request('c1')).toBe('ignored');
    release(withActiveCape(base(), 'c1')); await p;
    expect(h.shownLog).toEqual(['c1', 'c1']);
  });
});
describe('capeError', () => {
  const e = (code: string, message = '', needsRelogin = false) => capeError({ code, message, needsRelogin });
  it('maps the specific codes', () => {
    expect(e('skin.cape_not_applied').key).toBe('err.skin.cape_not_applied');
    expect(e('skin.upload_failed').key).toBe('err.skin.upload_failed');
    expect(e('skin.timeout')).toMatchObject({ key: 'err.skin.timeout', retry: true });
    expect(e('x', '', true)).toMatchObject({ key: 'skins.capeRelogin', relogin: true });
    expect(e('whatever').key).toBe('skins.capeFailed');
  });
  it('rate limit shows the seconds when the message has them', () => {
    expect(e('skin.rate_limited', 'retry after 12s')).toMatchObject({ key: 'err.skin.rate_limited', vars: { s: 12 } });
    expect(e('skin.rate_limited', 'slow down').key).toBe('err.skin.rate_limited_nosec');
    expect(retrySeconds('retry_after=30')).toBe(30);
  });
});
