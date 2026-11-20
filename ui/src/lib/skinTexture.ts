// Loading skin and cape images as WebGL textures. WebGL refuses cross-origin images unless the response allows them (CORS), and the
// custom protocol / the dev-server bridge are other origins than the page. The packaged CSP (`connect-src ipc: http://ipc.localhost`)
// blocks fetch() of everything else, so NO fetch / createImageBitmap here (guarded by cspGuard.test.ts). The order tried:
//   1. <img crossOrigin="anonymous">  (needs Access-Control-Allow-Origin for the app origin on the mangoimg:// response; img-src allows the scheme)
//   2. a plain <img>                  (tainted: fine for 2D canvas drawing, NOT for WebGL; the viewer uses the flat fallback)
export type TexPath = 'image-cors' | 'local' | 'plain';
export interface LoadedTexture { source: HTMLImageElement; path: TexPath; width: number; height: number }

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
  if (opts.signal?.aborted) throw new Error('aborted');
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
