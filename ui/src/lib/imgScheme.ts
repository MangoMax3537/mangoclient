// The backend hands every image to the webview as `mangoimg://<kind>/<arg>` (instances, mods, Modrinth, heads, blocks, worlds,
// skins). WebKitGTK (Linux) and WKWebView (macOS) load that scheme directly. WebView2 (Windows) and Android only dispatch custom
// schemes as `http(s)://<scheme>.localhost/...`, so a raw `mangoimg://` URL never loads there: every image fell back to the
// MangoClient icon. The Tauri transport converts every string of every result and event on those platforms, and converts the
// other way for command arguments, so the backend contract stays `mangoimg://` everywhere.

const RAW = 'mangoimg://';
const LOCALHOST = /^https?:\/\/mangoimg\.localhost\//;

/** WebView2 / Android serve custom schemes from `<scheme>.localhost`. Decided by the user agent (a dev page runs on localhost:5173). */
export const servesSchemeOnLocalhost = (ua: string): boolean => /Windows|Android/i.test(ua);

/** `mangoimg://k/r` -> `http(s)://mangoimg.localhost/k/r` (https when the page itself is https, as with `useHttpsScheme`). */
export function toLocalhostImg(v: string, https = false): string {
  return v.startsWith(RAW) ? `${https ? 'https' : 'http'}://mangoimg.localhost/${v.slice(RAW.length)}` : v;
}

/** The inverse: what the backend understands. */
export function toRawImg(v: string): string {
  return LOCALHOST.test(v) ? RAW + v.replace(LOCALHOST, '') : v;
}

/** Applies `fn` to every string inside `value` (objects, arrays, nested); everything else is kept as it is. */
export function mapStrings<T>(value: T, fn: (s: string) => string): T {
  const walk = (v: unknown): unknown => {
    if (typeof v === 'string') return fn(v);
    if (Array.isArray(v)) return v.map(walk);
    if (v && typeof v === 'object') return Object.fromEntries(Object.entries(v).map(([k, x]) => [k, walk(x)]));
    return v;
  };
  return walk(value) as T;
}
