import { describe, expect, it } from 'vitest';
import { mapStrings, servesSchemeOnLocalhost, toLocalhostImg, toRawImg } from './imgScheme';

const WIN = 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36 Edg/131.0.0.0';
const MAC = 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko)';
const LINUX = 'Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/605.1.15 (KHTML, like Gecko)';

describe('image scheme per webview', () => {
  it('only WebView2 (Windows) and Android serve custom schemes on <scheme>.localhost', () => {
    expect(servesSchemeOnLocalhost(WIN)).toBe(true);
    expect(servesSchemeOnLocalhost('Mozilla/5.0 (Linux; Android 14)')).toBe(true);
    expect(servesSchemeOnLocalhost(MAC)).toBe(false);
    expect(servesSchemeOnLocalhost(LINUX)).toBe(false);
  });
  it('converts mangoimg:// both ways and keeps the query', () => {
    expect(toLocalhostImg('mangoimg://block/dirt/top?v=3')).toBe('http://mangoimg.localhost/block/dirt/top?v=3');
    expect(toLocalhostImg('mangoimg://head/abc', true)).toBe('https://mangoimg.localhost/head/abc');
    expect(toRawImg('http://mangoimg.localhost/block/dirt/top?v=3')).toBe('mangoimg://block/dirt/top?v=3');
    expect(toRawImg('https://mangoimg.localhost/head/abc')).toBe('mangoimg://head/abc');
    expect(toLocalhostImg('https://cdn.modrinth.com/x.png')).toBe('https://cdn.modrinth.com/x.png');
    expect(toRawImg('http://mangoimg.localhost.evil.com/x')).toBe('http://mangoimg.localhost.evil.com/x');
  });
  it('walks nested results and event payloads, leaving non-strings alone', () => {
    const state = { profiles: [{ coverUrl: 'mangoimg://cover/p1', playTimeMs: 5, icon: null, tags: ['mangoimg://x'] }], ok: true };
    const out = mapStrings(state, (s) => toLocalhostImg(s));
    expect(out).toEqual({ profiles: [{ coverUrl: 'http://mangoimg.localhost/cover/p1', playTimeMs: 5, icon: null, tags: ['http://mangoimg.localhost/x'] }], ok: true });
    expect(mapStrings(out, toRawImg)).toEqual(state);
  });
});
