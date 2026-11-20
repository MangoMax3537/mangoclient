// Minimal W3C WebDriver client over fetch (zero dependencies).

export class WdError extends Error {
  constructor(status, body) {
    const v = body && body.value;
    super(`WebDriver ${status}: ${v && v.error ? `${v.error}: ${v.message}` : JSON.stringify(body)}`);
    this.status = status;
    this.code = v && v.error;
  }
}

const ELEMENT_KEY = 'element-6066-11e4-a52e-4f735466cecf';

export class Session {
  constructor(base, id) {
    this.base = base;
    this.id = id;
  }

  static async create(base, capabilities) {
    const r = await raw('POST', `${base}/session`, { capabilities: { alwaysMatch: capabilities } });
    return new Session(base, r.sessionId);
  }

  async cmd(method, path, body) {
    return raw(method, `${this.base}/session/${this.id}${path}`, body);
  }

  async quit() {
    try {
      await raw('DELETE', `${this.base}/session/${this.id}`);
    } catch {
      /* already gone */
    }
  }

  title() { return this.cmd('GET', '/title'); }
  url() { return this.cmd('GET', '/url'); }
  screenshot() { return this.cmd('GET', '/screenshot'); }
  windowRect() { return this.cmd('GET', '/window/rect'); }

  /** Run a script; `args` are passed as `arguments`. Element refs are unwrapped on the way out by callers. */
  exec(script, args = []) {
    return this.cmd('POST', '/execute/sync', { script, args });
  }

  /** Async script: the script's last argument is the completion callback. */
  execAsync(script, args = []) {
    return this.cmd('POST', '/execute/async', { script, args });
  }

  async find(using, value) {
    const els = await this.cmd('POST', '/elements', { using, value });
    return els.map((e) => e[ELEMENT_KEY]);
  }

  /** Key chord through the W3C actions API: e.g. keys(['\uE009', '5']) = Ctrl+5 (modifiers first, released in reverse). */
  async keys(chord) {
    const seq = [...chord.map((value) => ({ type: 'keyDown', value })), ...[...chord].reverse().map((value) => ({ type: 'keyUp', value }))];
    return this.cmd('POST', '/actions', { actions: [{ type: 'key', id: 'kbd', actions: seq }] });
  }

  click(id) { return this.cmd('POST', `/element/${id}/click`, {}); }
  clear(id) { return this.cmd('POST', `/element/${id}/clear`, {}); }
  sendKeys(id, text) { return this.cmd('POST', `/element/${id}/value`, { text }); }
  text(id) { return this.cmd('GET', `/element/${id}/text`); }
  elementObj(id) { return { [ELEMENT_KEY]: id }; }
}

async function raw(method, url, body) {
  const res = await fetch(url, {
    method,
    headers: body === undefined ? {} : { 'content-type': 'application/json' },
    body: body === undefined ? undefined : JSON.stringify(body),
    signal: AbortSignal.timeout(method === 'POST' && url.endsWith('/session') ? 45_000 : 120_000),
  });
  const text = await res.text();
  let json;
  try {
    json = text ? JSON.parse(text) : {};
  } catch {
    json = { raw: text };
  }
  if (!res.ok || (json.value && json.value.error)) throw new WdError(res.status, json);
  const v = json.value;
  if (v && v.sessionId) return v; // new session
  return v;
}

export const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

export async function waitFor(fn, { timeout = 15000, interval = 200, what = 'condition' } = {}) {
  const end = Date.now() + timeout;
  let last;
  for (;;) {
    try {
      const v = await fn();
      if (v) return v;
    } catch (e) {
      last = e;
    }
    if (Date.now() > end) throw new Error(`timeout waiting for ${what}${last ? ` (last error: ${last.message})` : ''}`);
    await sleep(interval);
  }
}
