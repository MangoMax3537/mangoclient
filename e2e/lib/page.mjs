// In-page helpers: collectors, Tauri IPC calls, and text/aria-label based element lookup.
import { writeFileSync } from 'node:fs';
import { waitFor, sleep } from './wd.mjs';

const COLLECTORS = `
if (!window.__e2e) {
  const c = (window.__e2e = { errors: [], csp: [], logs: [], events: [] });
  window.addEventListener('error', (e) => c.errors.push('error: ' + e.message + ' @' + e.filename + ':' + e.lineno));
  window.addEventListener('unhandledrejection', (e) => {
    const r = e.reason;
    c.errors.push('unhandledrejection: ' + (r && (r.message || JSON.stringify(r)) || String(r)));
  });
  document.addEventListener('securitypolicyviolation', (e) => c.csp.push(e.violatedDirective + ' ' + e.blockedURI));
  for (const k of ['error', 'warn']) {
    const o = console[k];
    console[k] = (...a) => {
      c.logs.push(k + ': ' + a.map((x) => (x && x.message) || String(x)).join(' '));
      o.apply(console, a);
    };
  }
}
return true;`;

export class Page {
  constructor(session, outDir) {
    this.s = session;
    this.out = outDir;
  }

  async init() {
    await this.s.cmd('POST', '/timeouts', { script: 90000, pageLoad: 30000, implicit: 0 });
    await this.s.exec(COLLECTORS);
  }

  exec(body, args) { return this.s.exec(body, args); }

  collected() { return this.exec('return window.__e2e || null;'); }

  /** Call a Tauri command through the real IPC bridge. Resolves to { ok } or { err }. */
  invoke(cmd, args = {}) {
    return this.s.execAsync(
      `const done = arguments[arguments.length - 1];
       try {
         window.__TAURI_INTERNALS__.invoke(arguments[0], arguments[1]).then(
           (v) => done({ ok: v === undefined ? null : v }),
           (e) => done({ err: typeof e === 'string' ? e : JSON.stringify(e) }));
       } catch (e) { done({ err: 'sync: ' + e.message }); }`,
      [cmd, args],
    );
  }

  async shot(name) {
    const b64 = await this.s.screenshot();
    const file = `${this.out}/${name}.png`;
    writeFileSync(file, Buffer.from(b64, 'base64'));
    return file;
  }

  /**
   * First visible element matching `selector` whose aria-label or text matches `re`.
   * Returns a WebDriver element id or null.
   */
  async findText(re, selector = 'button, [role=button], a, [role=radio], [role=tab], [role=option], li') {
    const ids = await this.s.exec(
      `const re = new RegExp(arguments[0], 'i');
       const vis = (e) => { const r = e.getBoundingClientRect(); const cs = getComputedStyle(e); return r.width > 0 && r.height > 0 && cs.visibility !== 'hidden' && cs.display !== 'none'; };
       const hit = [...document.querySelectorAll(arguments[1])].filter((e) => vis(e) && !e.disabled && (re.test(e.getAttribute('aria-label') || '') || re.test((e.textContent || '').trim())));
       // prefer the innermost match (a li containing a matching button is not the target)
       const inner = hit.filter((e) => !hit.some((o) => o !== e && e.contains(o)));
       return inner[0] || null;`,
      [re.source, selector],
    );
    return ids ? Object.values(ids)[0] : null;
  }

  async click(re, opts = {}) {
    const id = await waitFor(() => this.findText(re, opts.selector), { timeout: opts.timeout ?? 10000, what: `clickable ${re}` });
    await this.s.click(id);
    await sleep(opts.settle ?? 150);
    return id;
  }

  async has(re, selector) {
    return !!(await this.findText(re, selector));
  }

  /** Visible text of the whole body, lower weight than a screenshot but diff-able. */
  bodyText() { return this.exec('return document.body.innerText;'); }

  async byCss(css) {
    const ids = await this.s.find('css selector', css);
    return ids[0] || null;
  }
}
