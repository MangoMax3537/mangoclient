#!/usr/bin/env node
// End-to-end harness: drives the REAL built MangoClient binary through WebKitWebDriver.
// Usage: node e2e/run.mjs [--binary <path>] [--port N] [--out <dir>] [--keep]
import { spawn, spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync, chmodSync, openSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { Session, sleep, waitFor } from './lib/wd.mjs';
import { Page } from './lib/page.mjs';
import { alive, descendants, envOf, exeOf, kill, pids, pidsByExeName, statOf, waitExit } from './lib/proc.mjs';

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const argv = process.argv.slice(2);
const opt = (name, dflt) => {
  const i = argv.indexOf(name);
  return i >= 0 && argv[i + 1] ? argv[i + 1] : dflt;
};
const BINARY = resolve(opt('--binary', join(ROOT, 'src-tauri/target/debug/mangoclient')));
const PORT = Number(opt('--port', '4455'));
const STAMP = new Date().toISOString().replace(/[:.]/g, '-');
const OUT = resolve(opt('--out', join(ROOT, 'target', 'e2e', STAMP)));
const HOME_DIR = join(OUT, 'data'); // fresh MANGOCLIENT_HOME
const KEEP = argv.includes('--keep');

const results = []; // { id, name, status, note }
const findings = []; // real application bugs noticed along the way
class SkipError extends Error {}
let skipReason = '';
let driverPid = 0;
let appPid = 0;
let session = null;
let page = null;
const t0 = Date.now();
const log = (...a) => console.log(`[${((Date.now() - t0) / 1000).toFixed(1).padStart(5)}s]`, ...a);

function check(cond, msg) {
  if (!cond) throw new Error(msg);
}

async function scenario(id, name, fn) {
  log(`${id} ${name} ...`);
  try {
    const note = await fn();
    results.push({ id, name, status: 'PASS', note: note || '' });
    log(`${id} PASS ${note || ''}`);
  } catch (e) {
    if (e instanceof SkipError) {
      skip(id, name, e.message);
      return;
    }
    let shot = '';
    try {
      if (page && alive(appPid)) shot = await page.shot(`FAIL-${id}`);
    } catch {
      /* window gone */
    }
    results.push({ id, name, status: 'FAIL', note: e.message + (shot ? `  [screenshot ${shot}]` : '') });
    log(`${id} FAIL ${e.message}`);
  }
}

function skip(id, name, why) {
  results.push({ id, name, status: 'SKIP', note: why });
  log(`${id} SKIP ${why}`);
}

// ---------------------------------------------------------------- setup

function preflight() {
  check(existsSync(BINARY), `binary not found: ${BINARY} (build: cargo build --manifest-path src-tauri/Cargo.toml --features tauri/custom-protocol)`);
  // The single-instance lock is global per appId, not per MANGOCLIENT_HOME: a running real launcher would receive our launches.
  const running = pidsByExeName('mangoclient');
  check(running.length === 0, `another mangoclient is already running (pids ${running.join(',')}); close it first, the single-instance lock is global and the test must not touch it`);
}

async function startDriver() {
  const logFd = openSync(join(OUT, 'driver.log'), 'a');
  const p = spawn('WebKitWebDriver', ['-p', String(PORT)], { stdio: ['ignore', logFd, logFd] });
  driverPid = p.pid;
  p.on('error', (e) => log('driver spawn error', e.message));
  await waitFor(async () => (await fetch(`http://127.0.0.1:${PORT}/status`)).ok, { timeout: 10000, what: 'WebKitWebDriver /status' });
}

/** A wrapper script is the "binary" so the real exit status of the app can be recorded (the driver swallows it). */
function writeWrapper() {
  const w = join(OUT, 'app-wrapper.sh');
  writeFileSync(
    w,
    `#!/bin/sh\n"${BINARY}" "$@" >>"${OUT}/app.log" 2>&1\necho $? >"${OUT}/exitcode"\n`,
  );
  chmodSync(w, 0o755);
  return w;
}

function findAppPid() {
  return pids().find((p) => exeOf(p) === BINARY && envOf(p).MANGOCLIENT_HOME === HOME_DIR) || 0;
}

// ---------------------------------------------------------------- scenarios

async function s1() {
  const title = await session.title();
  check(title === 'MangoClient', `window title is ${JSON.stringify(title)}, expected "MangoClient"`);
  const url = await session.url();
  const rect = await session.windowRect();
  await waitFor(() => page.exec(`return document.readyState === 'complete' && !!document.getElementById('app') && document.getElementById('app').children.length > 0;`), { what: 'document loaded and #app mounted' });
  const c = await page.collected();
  check(c.errors.length === 0, `page errors: ${c.errors.join(' | ')}`);
  const bad = c.logs.filter((l) => l.startsWith('error:'));
  check(bad.length === 0, `console.error output: ${bad.join(' | ')}`);
  return `url ${url}, window ${rect.width}x${rect.height}; note: collectors attach after load, boot-time errors surface via S2`;
}

async function s2() {
  const r = await page.invoke('app_state');
  check(!r.err, `app_state rejected: ${r.err}`);
  const st = r.ok;
  check(st && typeof st.config === 'object' && st.config !== null, 'app_state.config is not an object');
  check(Array.isArray(st.profiles), 'app_state.profiles is not an array');
  check(Array.isArray(st.accounts), 'app_state.accounts is not an array');
  // the UI's own boot must have rendered (no boot error panel, no skeleton-only screen)
  const ui = await page.exec(`return { text: document.body.innerText.length, busy: document.querySelectorAll('[aria-busy=true]').length, dialog: !!document.querySelector('.ob'), alert: [...document.querySelectorAll('[role=alert]')].map((e) => e.innerText).join(' | ') };`);
  check(ui.text > 20, 'UI rendered almost no text');
  check(!ui.alert, `UI shows an alert at boot: ${ui.alert}`);
  // least privilege: a plugin that is registered but not granted to the window must be refused
  const denied = await page.invoke('plugin:clipboard-manager|read_text');
  check(denied.err && /not allowed|permission|denied/i.test(denied.err), `ungranted plugin command was not refused: ${JSON.stringify(denied)}`);
  return `config/profiles(${st.profiles.length})/accounts(${st.accounts.length}) ok; ungranted plugin call refused; first run onboarding shown=${ui.dialog}`;
}

let probeAccount = null;

async function s3() {
  // Subscribe through the real event plugin (what @tauri-apps/api `listen` does), then run a command that makes the core emit.
  // servers:pinged is the harmless emitter (a ping of the seeded partner servers; account_add_offline itself emits nothing).
  const sub = await page.s.execAsync(
    `const done = arguments[arguments.length - 1]; const ti = window.__TAURI_INTERNALS__;
     const h = ti.transformCallback((e) => window.__e2e.events.push(e));
     ti.invoke('plugin:event|listen', { event: 'servers:pinged', target: { kind: 'Any' }, handler: h }).then((id) => done({ ok: id }), (e) => done({ err: String(e && e.message || JSON.stringify(e)) }));`,
  );
  check(!sub.err, `event listen refused under the reduced capability: ${sub.err}`);
  const r = await page.invoke('account_add_offline', { name: 'E2EProbe' });
  check(!r.err, `account_add_offline failed: ${r.err}`);
  probeAccount = r.ok;
  const ping = await page.invoke('servers_ping_all');
  check(!ping.err, `servers_ping_all failed: ${ping.err}`);
  await waitFor(() => page.exec('return window.__e2e.events.length > 0;'), { timeout: 30000, what: 'servers:pinged event' });
  const ev = await page.exec('return window.__e2e.events[0];');
  check(ev && ev.event === 'servers:pinged', `unexpected event envelope ${JSON.stringify(ev)}`);
  const c = await page.collected();
  check(c.errors.length === 0, `page errors during event test: ${c.errors.join(' | ')}`);
  return `servers:pinged delivered (${(await page.exec('return window.__e2e.events.length;'))} so far, first id ${ev.payload && ev.payload.id})`;
}

async function s4() {
  check(probeAccount && probeAccount.headUrl, 'no probe account from S3 (headUrl missing)');
  const load = (url) =>
    page.s.execAsync(
      `const done = arguments[arguments.length - 1]; const i = new Image();
       i.onload = () => done({ ok: true, w: i.naturalWidth, h: i.naturalHeight });
       i.onerror = () => done({ ok: false });
       setTimeout(() => done({ ok: false, timeout: true }), 20000);
       i.src = arguments[0];`,
      [url],
    );
  const head = await load(probeAccount.headUrl);
  check(head.ok && head.w > 0, `head image did not load via ${probeAccount.headUrl}: ${JSON.stringify(head)}`);
  // Outside the allowed roots: a real PNG in the scratch dir (not under the data root), addressed through the f/ route.
  const outside = join(OUT, 'outside.png');
  writeFileSync(outside, Buffer.from('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==', 'base64'));
  const b64 = Buffer.from(outside).toString('base64url');
  const refused = [
    `mangoimg://f/${b64}`,
    `mangoimg://cover/..%2F..%2Fetc`,
    `mangoimg://cover/../../outside`,
    `mangoimg://f/${Buffer.from('/etc/passwd').toString('base64url')}`,
  ];
  for (const u of refused) {
    const r = await load(u);
    check(!r.ok, `mangoimg URL outside the allowed roots was served: ${u}`);
  }
  // Also confirm the UI's own <img> tags using the scheme rendered.
  const imgs = await page.exec(`return [...document.images].filter((i) => i.src.startsWith('mangoimg:')).map((i) => i.naturalWidth);`);
  return `head ${head.w}x${head.h} loaded; ${refused.length} outside-root URLs refused; UI mangoimg <img>: ${imgs.length} (${imgs.filter((w) => w > 0).length} decoded)`;
}

async function s5() {
  await page.exec(`window.__csp_ran = 0; const s = document.createElement('script'); s.textContent = 'window.__csp_ran = 1'; document.head.appendChild(s); return true;`);
  await sleep(300);
  const ran = await page.exec('return window.__csp_ran;');
  check(ran === 0, 'injected inline script EXECUTED (CSP not enforced)');
  const f = await page.s.execAsync(
    `const done = arguments[arguments.length - 1];
     fetch('https://example.com/', { mode: 'no-cors' }).then(() => done({ blocked: false }), (e) => done({ blocked: true, e: String(e) }));`,
  );
  check(f.blocked, 'fetch(https://example.com) from the page was NOT blocked');
  const img = await page.s.execAsync(
    `const done = arguments[arguments.length - 1]; const i = new Image(); i.onload = () => done(false); i.onerror = () => done(true); setTimeout(() => done('timeout'), 8000); i.src = 'https://example.com/favicon.ico';`,
  );
  check(img === true, `remote <img> from https://example.com was not blocked (${img})`);
  const c = await page.collected();
  check(c.csp.length > 0, 'no securitypolicyviolation event was observed');
  return `inline script, fetch and remote img blocked; violations: ${[...new Set(c.csp.map((x) => x.split(' ')[0]))].join(', ')}`;
}

async function s6() {
  const cspBaseline = (await page.collected()).csp.length;
  const onboarding = await page.exec(`return !!document.querySelector('.ob');`);
  check(onboarding, 'onboarding is not shown on a fresh data dir');
  await page.shot('s6-01-onboarding-welcome');
  // welcome -> language
  await page.click(/^(Get started|Los geht|Start|Loslegen)/i, { selector: '.nav .btn.accent' }).catch(async () => page.click(/./, { selector: '.nav .btn.accent' }));
  await page.click(/^English$/, { selector: '[role=radio]' });
  await page.shot('s6-02-onboarding-language');
  await page.click(/./, { selector: '.nav .btn.accent' });
  // account: add an offline account through the form
  const input = await waitFor(() => page.byCss('#ob-name'), { what: 'offline name field' });
  await page.s.sendKeys(input, 'E2EUiUser');
  await page.click(/./, { selector: 'form button[type=submit]' });
  await waitFor(() => page.exec(`return document.body.innerText.includes('E2EUiUser');`), { timeout: 10000, what: 'new offline account in the list' });
  await sleep(500);
  await page.shot('s6-03-onboarding-account');
  await page.click(/./, { selector: '.nav .btn.accent' });
  // RAM
  await page.click(/recommended|empfohlen/i, { selector: 'button' });
  await page.shot('s6-04-onboarding-ram');
  await page.click(/./, { selector: '.nav .btn.accent' });
  await page.click(/./, { selector: '.nav .btn.accent' }); // finish
  await waitFor(() => page.exec(`return !document.querySelector('.ob');`), { what: 'onboarding closed' });
  const st = (await page.invoke('app_state')).ok;
  check(st.config.firstRunDone === true, 'config.firstRunDone not persisted after onboarding');
  check(st.accounts.some((a) => a.name === 'E2EUiUser'), 'UI-created account missing from app_state');
  await sleep(600);
  await page.shot('s6-05-home-empty');

  // new profile (Vanilla)
  await page.click(/^(Library|Bibliothek)$/, { selector: 'button[aria-label]' });
  await page.click(/^(New profile|Neues Profil)$/, { selector: 'button' });
  const name = await waitFor(() => page.byCss('#np-name'), { what: 'new-profile name field' });
  await sleep(700); // dialog open animation: clicks while it moves land on neighbours
  await page.s.sendKeys(name, 'E2E Vanilla');
  await page.click(/^Vanilla$/i, { selector: '[role=radio]' }).catch(() => {}); // vanilla is the default loader
  await page.click(/^(Next|Weiter)$/, { selector: 'dialog button' });
  // version list is fed by the Mojang version manifest (read-only)
  const vbtn = await waitFor(() => page.findText(/^\d+\.\d+/, 'ul.vlist li button'), { timeout: 30000, what: 'a version in the list (Mojang manifest)' });
  await page.s.click(vbtn);
  await page.shot('s6-06-new-profile-version');
  for (let i = 0; i < 3; i++) {
    const create = await page.findText(/^(Create profile|Profil erstellen)$/, 'dialog button');
    if (create) {
      await page.s.click(create);
      break;
    }
    await page.click(/^(Next|Weiter)$/, { selector: 'dialog button' });
  }
  await waitFor(() => page.exec(`return !document.querySelector('dialog[open]');`), { timeout: 30000, what: 'new-profile dialog closed' });
  const st2 = (await page.invoke('app_state')).ok;
  check(st2.profiles.some((p) => p.name === 'E2E Vanilla'), 'profile "E2E Vanilla" missing from app_state after creation');
  await sleep(800);
  await page.shot('s6-07-library');

  await page.click(/^(Discover|Entdecken)$/, { selector: 'button[aria-label]' });
  await sleep(2500);
  const disc = await page.exec(`return { skel: document.querySelectorAll('[class*=skel], [aria-busy=true]').length, results: document.querySelectorAll('main li, main article, main [role=listitem]').length, text: document.body.innerText.length };`);
  await page.shot('s6-08-discover');
  check(disc.skel > 0 || disc.results > 0 || disc.text > 100, `Discover shows neither skeleton nor results: ${JSON.stringify(disc)}`);

  await page.click(/^(Settings|Einstellungen)$/, { selector: 'button[aria-label], a[aria-label]' });
  await sleep(1000);
  await page.shot('s6-09-settings');
  const c = await page.collected();
  check(c.errors.length === 0, `page errors while driving the UI: ${c.errors.join(' | ')}`);
  const bad = c.logs.filter((l) => l.startsWith('error:'));
  check(bad.length === 0, `console.error while driving the UI: ${bad.join(' | ')}`);
  const extraCsp = c.csp.length - cspBaseline;
  check(extraCsp === 0, `${extraCsp} unexpected CSP violation(s) from the app's own UI: ${c.csp.slice(cspBaseline).join(' | ')}`);
  if (c.logs.length) findings.push(`console warnings/errors seen: ${c.logs.slice(0, 5).join(' | ')}`);
  return `onboarding -> profile -> library/discover/settings; discover: ${disc.skel} skeleton/busy nodes, ${disc.results} result nodes; no console errors, no unexpected CSP violations`;
}

async function s7() {
  const second = spawn(BINARY, [], { env: { ...process.env, MANGOCLIENT_HOME: HOME_DIR }, stdio: 'ignore' });
  const started = Date.now();
  const exited = await new Promise((res) => {
    const timer = setTimeout(() => res(null), 8000);
    second.on('exit', (code, sig) => {
      clearTimeout(timer);
      res({ code, sig });
    });
  });
  if (!exited) {
    kill(second.pid, 'SIGKILL');
    throw new Error('second instance did not exit within 8s (single-instance not enforced)');
  }
  const ms = Date.now() - started;
  check(alive(appPid), 'the first instance died when the second one started');
  const title = await session.title(); // the first instance still answers
  check(title === 'MangoClient', 'first instance unresponsive after second start');
  return `second instance exited after ${ms} ms (code ${exited.code}), first alive`;
}

async function s8() {
  const webkitBefore = descendants(appPid).filter((p) => /WebKit/i.test(statOf(p)?.comm || ''));
  rmSync(join(OUT, 'exitcode'), { force: true });
  // WebDriver's "close window" only runs window.close() in the page, which WebKitGTK ignores for a native window, and the page
  // has no `core:window` permission (by design). So click the real close button: ask the X11 window manager (xdotool) to quit
  // the window. The harness forces GDK_BACKEND=x11 for this.
  let how;
  const wins = spawnSync('xdotool', ['search', '--pid', String(appPid), '--onlyvisible'], { encoding: 'utf8' });
  const id = (wins.stdout || '').split('\n').filter(Boolean).pop();
  if (!id) {
    skipReason = 'no X11 window found for the app (xdotool search --pid returned nothing); quit not exercised';
    throw new SkipError(skipReason);
  }
  spawnSync('xdotool', ['windowquit', id]);
  how = `xdotool windowquit ${id}`;
  const gone = await waitExit(appPid, 20000);
  if (!gone) throw new Error(`app still running 20 s after the window was closed (${how})`);
  await sleep(500);
  const code = existsSync(join(OUT, 'exitcode')) ? readFileSync(join(OUT, 'exitcode'), 'utf8').trim() : '(no exitcode file)';
  check(code === '0', `exit code ${code}, expected 0`);
  const stray = webkitBefore.filter((p) => alive(p));
  check(stray.length === 0, `stray WebKit processes after exit: ${stray.map((p) => `${p}:${statOf(p)?.comm}`).join(', ')}`);
  return `${how}: exit code 0; ${webkitBefore.length} WebKit helper processes all gone`;
}

// ---------------------------------------------------------------- main

async function cleanup() {
  if (session) await session.quit();
  await sleep(500);
  if (appPid && alive(appPid)) {
    kill(appPid, 'SIGTERM');
    if (!(await waitExit(appPid, 5000))) kill(appPid, 'SIGKILL');
  }
  // anything the driver launched with our data dir that survived (e.g. a failed session start)
  for (const p of pids()) if (exeOf(p) === BINARY && envOf(p).MANGOCLIENT_HOME === HOME_DIR) kill(p, 'SIGKILL');
  if (driverPid) kill(driverPid, 'SIGTERM');
  if (!KEEP) {
    // data dir only; screenshots and logs stay
    rmSync(HOME_DIR, { recursive: true, force: true });
  }
}

function summary() {
  console.log('\n==== E2E summary ====');
  const w = Math.max(...results.map((r) => r.name.length));
  for (const r of results) console.log(`${r.status.padEnd(5)} ${r.id}  ${r.name.padEnd(w)}  ${r.note}`);
  if (findings.length) {
    console.log('\nNotes:');
    for (const f of findings) console.log(` - ${f}`);
  }
  console.log(`\nartifacts: ${OUT}`);
  const failed = results.filter((r) => r.status === 'FAIL').length;
  console.log(failed ? `${failed} scenario(s) FAILED` : 'all executed scenarios passed');
  return failed;
}

async function main() {
  mkdirSync(OUT, { recursive: true });
  rmSync(HOME_DIR, { recursive: true, force: true });
  mkdirSync(HOME_DIR, { recursive: true });
  preflight();
  log(`binary ${BINARY}\n       data   ${HOME_DIR}\n       out    ${OUT}`);
  process.env.MANGOCLIENT_HOME = HOME_DIR; // inherited by the driver and the app it launches
  if (!argv.includes('--native')) {
    // XWayland: lets S8 close the window like the WM close button. Software-ish buffers: the dmabuf path fails on XWayland here.
    process.env.GDK_BACKEND = 'x11';
    process.env.WEBKIT_DISABLE_DMABUF_RENDERER = '1';
  }
  process.env.TAURI_WEBVIEW_AUTOMATION = 'true'; // tauri-runtime-wry: allow WebKit automation (what tauri-driver sets)
  await startDriver();
  const wrapper = writeWrapper();
  session = await Session.create(`http://127.0.0.1:${PORT}`, {
    'webkitgtk:browserOptions': { binary: wrapper, args: [] },
  });
  appPid = await waitFor(findAppPid, { timeout: 15000, what: 'app process' });
  log(`session ${session.id}, app pid ${appPid}`);
  page = new Page(session, OUT);
  await waitFor(async () => (await session.title()) === 'MangoClient', { timeout: 20000, what: 'window title' }).catch(() => {});
  await page.init();

  await scenario('S1', 'window, title, document, no errors', s1);
  await scenario('S2', 'IPC under reduced capability', s2);
  await scenario('S3', 'events (listen + emit observed)', s3);
  await scenario('S4', 'mangoimg:// protocol + root refusal', s4);
  await scenario('S5', 'CSP blocks inline script and network', s5);
  await scenario('S6', 'real UI: onboarding, profile, views', s6);
  await scenario('S7', 'single instance', s7);
  await scenario('S8', 'clean quit, exit 0, no strays', s8);
}

let code = 1;
try {
  await main();
} catch (e) {
  console.error('harness error:', e.message);
  results.push({ id: '--', name: 'harness', status: 'FAIL', note: e.message });
} finally {
  try {
    await cleanup();
  } catch (e) {
    console.error('cleanup error:', e.message);
  }
  code = results.length ? summary() : 1;
}
process.exit(code ? 1 : 0);
