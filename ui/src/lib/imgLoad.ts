// Image slot states shared by <Img>, BlockIcon and the galleries: loading (plain grey box) -> loaded (fades in) | error (real fallback);
// no URL at all is its own state ('none') and shows the fallback straight away.
export type ImgPhase = 'none' | 'loading' | 'loaded' | 'error';

export function imgPhase(src: string | null | undefined, loaded: boolean, failed: boolean): ImgPhase {
  if (!src) return 'none';
  if (failed) return 'error';
  return loaded ? 'loaded' : 'loading';
}

/** True for the phases in which the real fallback image (MangoClient icon) is shown. */
export const showsFallback = (p: ImgPhase): boolean => p === 'none' || p === 'error';

export interface ImgWatch { onload: (instant: boolean) => void; onerror: () => void }

/**
 * Svelte action for an <img>: reports `load` / `error` and also catches images that finished before the listener existed (cache hits),
 * reporting those as `instant` so they appear without a fade.
 */
export function imgWatch(node: HTMLImageElement, cb: ImgWatch & { src?: string | null }) {
  let cur = cb;
  const check = () => {
    if (!node.complete) return;
    if (node.naturalWidth > 0) cur.onload(true);
    else if (node.getAttribute('src')) cur.onerror();
  };
  const onload = () => cur.onload(false);
  const onerror = () => cur.onerror();
  node.addEventListener('load', onload);
  node.addEventListener('error', onerror);
  check();
  return {
    update(next: ImgWatch & { src?: string | null }) { const changed = next.src !== cur.src; cur = next; if (changed) queueMicrotask(check); },
    destroy() { node.removeEventListener('load', onload); node.removeEventListener('error', onerror); },
  };
}

/**
 * `img.decode()` with a ceiling. WebKitGTK can leave decode() pending for a large image on an element that is still invisible
 * (opacity 0) and never settle it, which kept the layer behind its grey skeleton for good. The `load` event already means the image
 * is usable, so decode only smooths the first paint: it may delay by `ms` at most, and any failure counts as done.
 */
export async function decodeSoon(el: Pick<HTMLImageElement, 'decode'> | null | undefined, ms = 600): Promise<void> {
  if (!el || typeof el.decode !== 'function') return;
  let timer: ReturnType<typeof setTimeout> | undefined;
  try {
    await Promise.race([el.decode().catch(() => undefined), new Promise<void>((r) => { timer = setTimeout(r, ms); })]);
  } catch { /* a decode failure still shows the image */ } finally { clearTimeout(timer); }
}
