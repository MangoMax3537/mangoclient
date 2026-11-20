// Loading skin and cape images as WebGL textures. WebGL refuses cross-origin images unless the response allows them (CORS), and the
// custom protocol / the dev-server bridge are other origins than the page. The order tried here:
//   1. fetch -> blob -> createImageBitmap   (works whenever the response has Access-Control-Allow-Origin for the app origin; no <img> taint)
//   2. <img crossOrigin="anonymous">         (same CORS requirement, but covers webviews where fetch() on the scheme is blocked)
//   3. a plain <img>                         (tainted: fine for 2D canvas drawing, NOT for WebGL; the viewer uses the flat fallback)
export type TexPath = 'fetch' | 'image-cors' | 'local' | 'plain';
export interface LoadedTexture { source: ImageBitmap | HTMLImageElement; path: TexPath; width: number; height: number }

const FETCH_TIMEOUT_MS = 15_000;

function imgLoad(url: string, cors: boolean): Promise<HTMLImageElement> {
  return new Promise((resolve, reject) => {
    const img = new Image();
    if (cors) img.crossOrigin = 'anonymous';
    img.onload = () => resolve(img);
    img.onerror = () => reject(new Error('image failed'));
    img.src = url;
  });
}

/** data: and blob: URLs are same-origin for our purposes: no CORS dance needed. */
const sameOriginish = (url: string): boolean => /^(data|blob):/.test(url);

export async function loadTexture(url: string, opts: { allowPlain?: boolean; signal?: AbortSignal } = {}): Promise<LoadedTexture> {
  if (!sameOriginish(url) && typeof fetch === 'function' && typeof createImageBitmap === 'function') {
    try {
      const ctl = new AbortController();
      const timer = setTimeout(() => ctl.abort(), FETCH_TIMEOUT_MS);
      opts.signal?.addEventListener('abort', () => ctl.abort(), { once: true });
      try {
        const res = await fetch(url, { signal: ctl.signal, mode: 'cors', credentials: 'omit' });
        if (!res.ok) throw new Error(`status ${res.status}`);
        const bmp = await createImageBitmap(await res.blob(), { premultiplyAlpha: 'none', colorSpaceConversion: 'none' });
        return { source: bmp, path: 'fetch', width: bmp.width, height: bmp.height };
      } finally { clearTimeout(timer); }
    } catch (e) { if (opts.signal?.aborted) throw e; /* try the next path */ }
  }
  try {
    const img = await imgLoad(url, !sameOriginish(url));
    return { source: img, path: sameOriginish(url) ? 'local' : 'image-cors', width: img.naturalWidth, height: img.naturalHeight };
  } catch (e) {
    if (!opts.allowPlain) throw e;
  }
  const img = await imgLoad(url, false);
  return { source: img, path: 'plain', width: img.naturalWidth, height: img.naturalHeight };
}

/** A texture that may be uploaded to WebGL (not tainted). */
export const glSafe = (t: LoadedTexture): boolean => t.path !== 'plain';
