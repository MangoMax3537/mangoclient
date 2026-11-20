// Optimistic cape change for the Skins page, as plain logic (no Svelte): the chosen cape becomes active at once (list + 3D model), the backend
// call runs with a timeout, and on any failure the previous state is restored and the caller reports a specific error. While one change is in
// flight every further request is ignored (double clicks, a second cape clicked in a hurry).
import { toAppError } from './api';
import type { Key } from './i18n';
import type { SkinInfo } from './types';

export const CAPE_TIMEOUT_MS = 20_000;

/** The info with `id` as the active cape (`null` = no cape). Unknown ids leave it unchanged. */
export function withActiveCape(info: SkinInfo, id: string | null): SkinInfo {
  if (id !== null && !info.capes.some((c) => c.id === id)) return info;
  const capes = info.capes.map((c) => ({ ...c, active: c.id === id }));
  return { ...info, capes, capeUrl: capes.find((c) => c.active)?.url ?? null };
}

export interface CapeError { key: Key; vars?: Record<string, string | number>; relogin: boolean; /** The toast offers Retry. */ retry?: boolean }
/** Whole seconds named in a rate-limit error (`message` such as "retry after 12s" / "retry_after=12"), else null. */
export function retrySeconds(message: string): number | null {
  const m = /(\d{1,5})(?:\.\d+)?\s*(?:s|sec|seconds)?\b/i.exec(message);
  return m ? Math.max(1, Number(m[1])) : null;
}
/** Maps a backend error to the sentence the toast shows: cape_not_applied, rate_limited (with seconds), upload_failed, relogin, anything else generic. */
export function capeError(e: unknown): CapeError {
  const err = toAppError(e);
  if (err.needsRelogin) return { key: 'skins.capeRelogin', relogin: true };
  if (err.code === 'skin.rate_limited') {
    const s = retrySeconds(err.message);
    return s === null ? { key: 'err.skin.rate_limited_nosec', relogin: false } : { key: 'err.skin.rate_limited', vars: { s }, relogin: false };
  }
  if (err.code === 'skin.timeout' || err.code === 'net.timeout') return { key: 'err.skin.timeout', relogin: false, retry: true };
  if (err.code === 'skin.cape_not_applied') return { key: 'err.skin.cape_not_applied', relogin: false };
  if (err.code === 'skin.upload_failed') return { key: 'err.skin.upload_failed', relogin: false };
  return { key: 'skins.capeFailed', relogin: false };
}

export interface CapeFlowDeps {
  current: () => SkinInfo | null;
  /** Show this info (optimistic state, the confirmed answer, or the rollback). */
  commit: (i: SkinInfo) => void;
  call: (capeId: string | null) => Promise<SkinInfo>;
  /** The id being saved (`null` = "no cape"), or `undefined` when nothing is. */
  saving: (id: string | null | undefined) => void;
  fail: (e: CapeError, raw: unknown) => void;
  timeoutMs?: number;
}
export type CapeRequestResult = 'ignored' | 'noop' | 'ok' | 'rolledBack';

export function createCapeFlow(d: CapeFlowDeps) {
  let inFlight = false;
  return {
    get busy(): boolean { return inFlight; },
    async request(id: string | null): Promise<CapeRequestResult> {
      if (inFlight) return 'ignored';
      const prev = d.current();
      if (!prev) return 'ignored';
      if ((prev.capes.find((c) => c.active)?.id ?? null) === id) return 'noop';
      inFlight = true;
      d.commit(withActiveCape(prev, id));
      d.saving(id);
      let timer: ReturnType<typeof setTimeout> | undefined;
      try {
        const timeout = new Promise<never>((_, rej) => { timer = setTimeout(() => rej({ code: 'net.timeout', message: 'timeout', needsRelogin: false }), d.timeoutMs ?? CAPE_TIMEOUT_MS); });
        d.commit(await Promise.race([d.call(id), timeout]));
        return 'ok';
      } catch (e) {
        d.commit(prev);
        d.fail(capeError(e), e);
        return 'rolledBack';
      } finally {
        clearTimeout(timer);
        inFlight = false;
        d.saving(undefined);
      }
    },
  };
}
