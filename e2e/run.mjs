#!/usr/bin/env node
// End-to-end harness: drives the REAL built MangoClient binary through WebKitWebDriver.
// Usage: node e2e/run.mjs [--binary <path>] [--port N] [--out <dir>] [--keep]
import { spawn, spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, readFileSync, readdirSync, rmSync, writeFileSync, chmodSync, openSync } from 'node:fs';
import { homedir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { Session, sleep, waitFor } from './lib/wd.mjs';
import { Page } from './lib/page.mjs';
import { alive, descendants, envOf, exeOf, kill, pids, pidsByExeName, statOf, waitExit } from './lib/proc.mjs';
import * as x11 from './lib/x11.mjs';
import { decodePng, meanDiff, regionStats } from './lib/png.mjs';

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
const ONLY = opt('--only', '') ? new Set(opt('--only', '').split(',').map((x) => x.trim().toUpperCase())) : null; // e.g. --only S9A,S10
const wanted = (id) => !ONLY || ONLY.has(id.toUpperCase());
// Prepared data directory (downloaded 1.21.4 vanilla + fabric, Java, assets, an offline account) for S9..S13. It is only ever COPIED.
const SEED = resolve(opt('--seed', process.env.MANGO_E2E_SEED || '/tmp/claude-1000/-home-henrik-Dokumente-mangoclient/5961dfad-8ec5-4608-b257-4d9c8b1bfc77/scratchpad/mango-home'));
const COPIES = []; // data copies under ~/.cache/mango-e2e-<scenario>, deleted at the end unless --keep
let curHome = HOME_DIR; // MANGOCLIENT_HOME of the app instance that `session` drives

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
  if (!wanted(id)) return;
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
function writeWrapper(tag = '', home = HOME_DIR) {
  const w = join(OUT, `app-wrapper${tag}.sh`);
  // MANGOCLIENT_HOME is exported here (not inherited): the driver was started once, with the S1-S8 data dir
  writeFileSync(
    w,
    `#!/bin/sh\nexport MANGOCLIENT_HOME="${home}"\n"${BINARY}" "$@" >>"${OUT}/app${tag}.log" 2>&1\necho $? >"${OUT}/exitcode${tag}"\n`,
  );
  chmodSync(w, 0o755);
  return w;
}

function findAppPid() {
  return pids().find((p) => exeOf(p) === BINARY && envOf(p).MANGOCLIENT_HOME === curHome) || 0;
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
  // the RAM step was removed: the next step is the finish step
  await page.shot('s6-04-onboarding-finish');
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

// ---------------------------------------------------------------- S9..S13: lifecycle, 3D skin, title bar, cycling, visual survey
// They run on COPIES of a prepared data dir (--seed / MANGO_E2E_SEED) under ~/.cache/mango-e2e-<scenario>. accounts.json of the copy
// holds the offline account and the profiles only (the seed's Microsoft record is dropped, never copied). Telemetry is switched off.

class Abort extends Error {}
/** Poll `fn` (may be async) until it returns something truthy; errors thrown by `fn` abort at once. */
async function until(fn, { timeout = 15000, interval = 250, what = 'condition' } = {}) {
  const end = Date.now() + timeout;
  for (;;) {
    const v = await fn();
    if (v) return v;
    if (Date.now() > end) throw new Error(`timeout (${Math.round(timeout / 1000)} s) waiting for ${what}`);
    await sleep(interval);
  }
}

function seedProblem() {
  if (!existsSync(join(SEED, 'accounts.json'))) return `seed data dir not found (${SEED}); pass --seed <dir> or MANGO_E2E_SEED`;
  return '';
}

function prepareCopy(tag) {
  const dest = join(homedir(), '.cache', `mango-e2e-${tag}`);
  rmSync(dest, { recursive: true, force: true });
  mkdirSync(dest, { recursive: true });
  COPIES.push(dest);
  for (const e of readdirSync(SEED)) {
    if (/^accounts\.json/.test(e) || /^config\.json/.test(e) || /\.log$/.test(e) || /\.bak$/.test(e)) continue; // rebuilt below / not needed
    const r = spawnSync('cp', ['-a', '--reflink=auto', join(SEED, e), join(dest, e)]);
    check(r.status === 0, `copying ${e} failed: ${r.stderr}`);
  }
  const j = JSON.parse(readFileSync(join(SEED, 'accounts.json'), 'utf8'));
  j.accounts = (j.accounts || []).filter((a) => a.type === 'offline');
  for (const a of j.accounts) delete a.encryptedCredentials;
  check(j.accounts.length > 0, 'seed has no offline account');
  writeFileSync(join(dest, 'accounts.json'), JSON.stringify(j), { mode: 0o600 });
  const van = (j.profiles || []).find((p) => p.name === 'Vanilla Test') || (j.profiles || []).find((p) => p.loader === 'vanilla' && p.mcVersion === '1.21.4');
  const cfg = JSON.parse(readFileSync(join(SEED, 'config.json'), 'utf8'));
  Object.assign(cfg, { language: 'en', firstRunDone: true, telemetry: false, hideOnLaunch: false, keepLauncherOpen: true, selectedAccount: j.accounts[0].uuid, selectedProfile: van ? van.id : cfg.selectedProfile });
  writeFileSync(join(dest, 'config.json'), JSON.stringify(cfg, null, 2), { mode: 0o600 });
  return dest;
}

/** Start the app on `home` through a fresh WebDriver session and make it the current one. */
async function startApp(tag, home) {
  curHome = home;
  rmSync(join(OUT, `exitcode${tag}`), { force: true });
  const wrapper = writeWrapper(tag, home);
  session = await Session.create(`http://127.0.0.1:${PORT}`, { 'webkitgtk:browserOptions': { binary: wrapper, args: [] } });
  appPid = await waitFor(findAppPid, { timeout: 20000, what: 'app process' });
  page = new Page(session, OUT);
  await waitFor(async () => (await session.title()) === 'MangoClient', { timeout: 20000, what: 'window title' }).catch(() => {});
  await page.init();
  await waitFor(() => page.exec(`return document.readyState === 'complete' && !!document.getElementById('app') && document.getElementById('app').children.length > 0;`), { what: '#app mounted' });
  log(`app '${tag}' started, pid ${appPid}`);
}

// S9 measures the REAL lifecycle, so by default it runs WITHOUT WebDriver/automation: with TAURI_WEBVIEW_AUTOMATION=true tauri-runtime-wry
// (wry `register_automation`) keeps a clone of the first WebView in the WebContext for the whole app lifetime, which pins the
// WebKitWebProcess even after the window is destroyed (a harness artefact, never present in a user run). Pass --s9-webdriver to use
// the automation path anyway. The launch is started with the app's own shortcut (Ctrl+Shift+P = play the selected profile).
const S9_NATIVE = !argv.includes('--s9-webdriver');
let nativeChild = null;
let nativeExit = null; // { code, sig } once the natively started app ended

/** Start the app WITHOUT automation on `home`, focus its window, leave `session`/`page` unset. */
async function startAppNative(tag, home) {
  curHome = home;
  nativeExit = null;
  const env = { ...process.env, MANGOCLIENT_HOME: home };
  delete env.TAURI_WEBVIEW_AUTOMATION;
  const out = openSync(join(OUT, `app${tag}.log`), 'a');
  nativeChild = spawn(BINARY, [], { env, stdio: ['ignore', out, out] });
  nativeChild.on('exit', (code, sig) => { nativeExit = { code, sig }; });
  appPid = nativeChild.pid;
  await until(() => x11.mainWindows(appPid, true).length > 0, { timeout: 30000, what: 'the main window (native start)' });
  await sleep(3500); // let the UI mount
  log(`app '${tag}' started natively (no automation), pid ${appPid}`);
}

/** Press a key combination in the main window (activate it first). */
function pressInWindow(keys) {
  const w = x11.mainWindows(appPid, true)[0];
  check(w, 'no visible window to send keys to');
  x11.activate(w);
  spawnSync('sleep', ['0.6']);
  spawnSync('xdotool', ['key', '--clearmodifiers', keys]);
}

/** Terminate whatever is left of the current app instance (game first), by PID only. */
async function endApp() {
  nativeChild = null;
  if (appPid && alive(appPid)) {
    for (const j of x11.javaChildren(appPid)) kill(j, 'SIGTERM');
    kill(appPid, 'SIGTERM');
    if (!(await waitExit(appPid, 6000))) kill(appPid, 'SIGKILL');
  }
  if (session) await session.quit();
  session = null;
  page = null;
}

const exitCodeOf = (tag) => (S9_NATIVE && nativeExit && /^-s9/.test(tag) ? String(nativeExit.code ?? nativeExit.sig) : existsSync(join(OUT, `exitcode${tag}`)) ? readFileSync(join(OUT, `exitcode${tag}`), 'utf8').trim() : '(no exitcode file)');
const snap = () => ({ win: x11.mainWindows(appPid, false).length, vis: x11.mainWindows(appPid, true).length, wk: x11.webkitChildren(appPid).length, java: x11.javaChildren(appPid), mem: x11.memOf(appPid) });
/** Remaining WebKit helpers: "WebKitWebProcess(pid) RSS x / PSS y; ..." (process role and memory, for the report). */
const helperInfo = () => x11.webkitChildren(appPid).map((p) => { const m = x11.memOf(p); return `${x11.commOf ? x11.commOf(p) : 'helper'}(${p}) RSS ${m.rss} / PSS ${m.pss ?? 'n/a'} MiB`; }).join('; ') || 'none';
const memStr = (m) => `RSS ${m.rss} MiB / PSS ${m.pss ?? 'n/a'} MiB`;
const rectInfo = (id) => (id ? x11.geometry(id) : null);

function logTail(home) {
  try {
    const d = join(home, 'logs');
    const f = readdirSync(d).filter((n) => n.endsWith('.log')).map((n) => join(d, n)).sort().pop();
    return f ? readFileSync(f, 'utf8').split('\n').slice(-6).join(' | ').slice(0, 600) : '(no launcher log)';
  } catch {
    return '(no launcher log)';
  }
}

/** Set hideOnLaunch=true / keepLauncherOpen=false, start the game for the Vanilla 1.21.4 profile, wait until the window is destroyed. */
async function bootLaunchHidden(home, tag) {
  let prof;
  if (S9_NATIVE) {
    const cfgPath = join(home, 'config.json');
    const cfg = JSON.parse(readFileSync(cfgPath, 'utf8'));
    Object.assign(cfg, { hideOnLaunch: true, keepLauncherOpen: false });
    writeFileSync(cfgPath, JSON.stringify(cfg, null, 2), { mode: 0o600 });
    await startAppNative(tag, home);
    prof = { id: cfg.selectedProfile };
  } else {
    await startApp(tag, home);
    const st = (await page.invoke('app_state')).ok;
    prof = st.profiles.find((p) => p.name === 'Vanilla Test') || st.profiles.find((p) => p.loader === 'vanilla' && p.mcVersion === '1.21.4');
    check(prof, 'no 1.21.4 Vanilla profile in the data copy');
    const acc = st.accounts.find((a) => a.type === 'offline');
    check(acc && st.accounts.every((a) => a.type === 'offline'), 'data copy must hold only the offline account');
    const r = await page.invoke('config_set', { patch: { hideOnLaunch: true, keepLauncherOpen: false, selectedAccount: acc.uuid, selectedProfile: prof.id } });
    check(!r.err, `config_set failed: ${r.err}`);
    const cfg = (await page.invoke('app_state')).ok.config;
    check(cfg.hideOnLaunch === true && cfg.keepLauncherOpen === false, `config not applied: hideOnLaunch=${cfg.hideOnLaunch} keepLauncherOpen=${cfg.keepLauncherOpen}`);
  }
  const before = snap();
  before.helpers = helperInfo();
  check(before.vis >= 1, 'no visible main window before the launch');
  if (S9_NATIVE) {
    pressInWindow('ctrl+shift+p');
  } else {
    await page.exec(
      `window.__e2e.launch = 'pending';
       window.__TAURI_INTERNALS__.invoke('game_launch', { profileId: arguments[0] }).then((v) => { window.__e2e.launch = { ok: v }; }, (e) => { window.__e2e.launch = { err: String((e && e.message) || JSON.stringify(e)) }; });
       return true;`,
      [prof.id],
    );
  }
  const t0l = Date.now();
  let java = 0;
  await until(
    async () => {
      java = x11.javaChildren(appPid)[0] || 0;
      if (java) return true;
      const l = S9_NATIVE ? null : await page.exec('return window.__e2e.launch;').catch(() => null);
      if (l && l.err) throw new Abort(`game_launch failed: ${l.err}`);
      return false;
    },
    { timeout: 300000, interval: 500, what: 'the java game process (needs Java + version manifest; offline uses what is cached)' },
  );
  log(`java pid ${java} after ${Math.round((Date.now() - t0l) / 1000)} s`);
  const tGone = await until(
    () => {
      if (!alive(java)) throw new Abort(`the game exited before the launcher reported it running; launcher log tail: ${logTail(home)}`);
      return x11.mainWindows(appPid, false).length === 0 ? Date.now() : 0;
    },
    { timeout: 240000, interval: 300, what: 'the main window to be destroyed after the game reported running' },
  );
  const gone = (await until(() => x11.webProcesses(appPid).length === 0, { timeout: 20000, what: 'the WebKitWebProcess to disappear' }).then(() => true, () => false));
  await sleep(3000);
  const after = snap();
  after.helpers = helperInfo();
  return { prof, java, before, after, webkitGone: gone, goneMs: tGone - t0l };
}

async function s9a() {
  const bad = seedProblem();
  if (bad) throw new SkipError(bad);
  if (!x11.hasXdotool()) throw new SkipError('xdotool not available');
  const home = prepareCopy('s9');
  const b = await bootLaunchHidden(home, '-s9a');
  check(b.webkitGone, `the WebKitWebProcess is still present 20 s after the window was destroyed (left: ${helperInfo()})`);
  check(alive(appPid), 'the main process died while the window was destroyed');
  check(alive(b.java), 'the game died while the window was destroyed');
  check(b.after.win === 0, `${b.after.win} main window(s) still exist in X11 after the destroy`);
  const note = [`window+WebKit gone while java ${b.java} runs; ${S9_NATIVE ? 'native' : 'automation'} run; before: main ${memStr(b.before.mem)}, helpers [${b.before.helpers}]; destroyed: main ${memStr(b.after.mem)}, helpers [${b.after.helpers}]`];
  // end the game from outside
  const tKill = Date.now();
  kill(b.java, 'SIGTERM');
  await until(() => !alive(b.java), { timeout: 30000, what: 'the game to exit after SIGTERM' });
  await until(() => x11.mainWindows(appPid, true).length > 0, { timeout: 30000, what: 'the window to be re-created after the game ended' });
  const backMs = Date.now() - tKill;
  const wkBack = await until(() => x11.webProcesses(appPid).length > 0, { timeout: 15000, what: 'the WebKitWebProcess to come back' }).then(() => true, () => false);
  note.push(`window re-created ${backMs} ms after SIGTERM (WebKit back: ${wkBack})`);
  check(backMs <= 10000, `window came back only after ${backMs} ms (expected <= 10 s after the game ended)`);
  check(wkBack, 'WebKit processes did not come back with the window');
  await sleep(1500);
  // can anything attach to the re-created window?
  let usable = null;
  const alt = { err: '' };
  if (S9_NATIVE) alt.err = 'native run without automation: no WebDriver session exists to attach';
  else try {
    const t = await Promise.race([session.title(), sleep(8000).then(() => { throw new Error('old session does not answer'); })]);
    usable = t === 'MangoClient' ? 'old session' : null;
  } catch (e) {
    alt.err = e.message;
  }
  if (!usable && !S9_NATIVE) {
    try {
      // WebKitWebDriver always spawns its OWN browser process: with the single-instance lock held this only pokes the running app
      const s2 = await Promise.race([
        Session.create(`http://127.0.0.1:${PORT}`, { 'webkitgtk:browserOptions': { binary: writeWrapper('-s9a2', home), args: [] } }),
        sleep(40000).then(() => { throw new Error('new session did not start within 40 s'); }),
      ]);
      session = s2;
      usable = 'new session';
    } catch (e) {
      alt.err += ` / ${e.message}`;
    }
  }
  if (usable) {
    page = new Page(session, OUT);
    await page.init();
    const st = await page.invoke('app_state');
    check(!st.err, `app_state failed in the re-created window: ${st.err}`);
    const gr = await page.invoke('game_running');
    check(!gr.err && Array.isArray(gr.ok) && gr.ok.length === 0, `game_running not empty after the game ended: ${JSON.stringify(gr)}`);
    note.push(`${usable} attached: app_state ok, game_running empty`);
  } else {
    findings.push(`S9a: the re-created window could not be inspected through WebDriver (${alt.err.trim()}): WebKitWebDriver spawns its own browser and cannot attach to a window the app re-created; app_state/game_running in that window are UNVERIFIED here (process/X11 level only). Verify by hand: after a hidden launch ends, the UI shows the idle profile with a Play button.`);
    note.push('UI of re-created window not inspectable via WebDriver');
  }
  // clean quit of the re-created window
  const w = x11.mainWindows(appPid, true).pop();
  x11.quitWindow(w);
  check(await waitExit(appPid, 20000), 'app did not quit after closing the re-created window with no game running');
  const code = exitCodeOf('-s9a');
  check(code === '0', `exit code ${code} after quit`);
  await endApp();
  return note.join('; ');
}

async function s9b() {
  const bad = seedProblem();
  if (bad) throw new SkipError(bad);
  if (!x11.hasXdotool()) throw new SkipError('xdotool not available');
  const home = COPIES.find((c) => c.endsWith('mango-e2e-s9')) || prepareCopy('s9');
  const b = await bootLaunchHidden(home, '-s9b');
  check(b.webkitGone && alive(appPid) && alive(b.java), 'precondition (destroyed window, app and game alive) not met');
  const note = [`destroyed in ${Math.round(b.goneMs / 1000)} s after launch start`];
  // single instance while the window is destroyed
  const second = spawn(BINARY, [], { env: { ...process.env, MANGOCLIENT_HOME: home }, stdio: 'ignore' });
  const exited = new Promise((res) => second.on('exit', (code, sig) => res({ code, sig })));
  const ex = await Promise.race([exited, sleep(10000).then(() => null)]);
  if (!ex) { kill(second.pid, 'SIGKILL'); throw new Error('second start did not exit within 10 s'); }
  await until(() => x11.mainWindows(appPid, true).length > 0, { timeout: 10000, what: 'the window to reappear after the second start' });
  check(alive(appPid) && alive(b.java), 'app or game died during the second start');
  note.push(`second start exit ${ex.code}, window back; ${memStr(snap().mem)}`);
  await sleep(2500);
  check(x11.mainWindows(appPid, true).length > 0, 'the restored window was hidden again (repeated running must not re-hide a window the user brought back)');
  // closing the window must not quit while the game runs
  x11.quitWindow(x11.mainWindows(appPid, true).pop());
  await sleep(5000);
  check(alive(appPid), 'closing the window during a running game QUIT the launcher');
  check(alive(b.java), 'the game died when the window was closed');
  note.push(`window closed during game: app alive, windows left ${x11.mainWindows(appPid, false).length}`);
  // the user starts the executable again: the window must come back
  const second2 = spawn(BINARY, [], { env: { ...process.env, MANGOCLIENT_HOME: home }, stdio: 'ignore' });
  await Promise.race([new Promise((res) => second2.on('exit', res)), sleep(10000)]);
  kill(second2.pid, 'SIGKILL');
  await until(() => x11.mainWindows(appPid, true).length > 0, { timeout: 10000, what: 'the window to reappear after closing it during the game and starting again' });
  // game ends while the user has the window open again
  kill(b.java, 'SIGTERM');
  await until(() => !alive(b.java), { timeout: 30000, what: 'the game to exit' });
  const quitEarly = await waitExit(appPid, 15000);
  if (quitEarly) {
    const code = exitCodeOf('-s9b');
    findings.push(`BUG candidate S9b: the user closed the window while a game ran, started the launcher again (window visible), then the game ended: the launcher QUIT by itself (exit ${code}) although the window was open. The held-back quit (exit_wanted) is not cleared when the window is shown again.`);
    note.push(`launcher quit by itself after the game ended although the window was re-opened (exit ${code}) -> see Notes`);
  } else {
    note.push('window stayed open after the game ended');
    x11.quitWindow(x11.mainWindows(appPid, true).pop());
    check(await waitExit(appPid, 20000), 'app did not quit after closing the window with no game running');
    const code = exitCodeOf('-s9b');
    check(code === '0', `exit code ${code} after quit`);
  }
  await endApp();
  return note.join('; ');
}

// ---- the UI run (S10, S11, S12, S13 share one app instance)
let uiReady = false;
async function ensureUi() {
  if (uiReady && appPid && alive(appPid)) return;
  const bad = seedProblem();
  if (bad) throw new SkipError(bad);
  if (!x11.hasXdotool()) throw new SkipError('xdotool not available (window geometry checks)');
  const home = prepareCopy('ui');
  await startApp('-ui', home);
  await sleep(1500);
  const st = (await page.invoke('app_state')).ok;
  check(st.config.firstRunDone && st.accounts.length > 0, 'data copy did not boot into the main UI');
  uiReady = true;
}

async function combo(key, { shift = false } = {}) {
  const chord = [''];
  if (shift) chord.push('');
  chord.push(key);
  try {
    await session.keys(chord);
    await sleep(250);
    return 'webdriver actions';
  } catch (e) {
    await page.exec(
      `document.dispatchEvent(new KeyboardEvent('keydown', { key: arguments[0], ctrlKey: true, shiftKey: arguments[1], bubbles: true, cancelable: true })); return true;`,
      [key, shift],
    );
    await sleep(250);
    return 'synthetic DOM event (actions API failed: ' + e.message.slice(0, 60) + ')';
  }
}
const currentNav = () => page.exec(`const b = document.querySelector('nav.rail button[aria-current="page"]'); return b ? b.getAttribute('aria-label') : null;`);
async function goView(key, label, shiftOrRail) {
  const how = await combo(key);
  if ((await currentNav()) === label) return `Ctrl+${key} (${how})`;
  await page.click(new RegExp(`^${label}$`), { selector: 'nav.rail button.nav' });
  await sleep(400);
  check((await currentNav()) === label, `could not navigate to ${label}`);
  return `rail click (Ctrl+${key} did not switch via ${how})`;
}
const rectOf = (css) => page.exec(`const e = document.querySelector(arguments[0]); if (!e) return null; const r = e.getBoundingClientRect(); return { x: r.x, y: r.y, w: r.width, h: r.height, vw: innerWidth, vh: innerHeight, dpr: devicePixelRatio };`, [css]);
async function shotImg(name) {
  const file = await page.shot(name);
  return { file, img: decodePng(readFileSync(file)) };
}

async function s10() {
  await ensureUi();
  await goView('1', 'Home');
  await page.exec(
    `const t = (window.__e2e.tex = []);
     const f = window.fetch; window.fetch = function (...a) { t.push('fetch ' + String(a[0]).slice(0, 50)); return f.apply(this, a); };
     if (window.createImageBitmap) { const c = window.createImageBitmap; window.createImageBitmap = function (...a) { t.push('createImageBitmap'); return c.apply(this, a); }; }
     const d = Object.getOwnPropertyDescriptor(HTMLImageElement.prototype, 'crossOrigin');
     if (d && d.set) Object.defineProperty(HTMLImageElement.prototype, 'crossOrigin', { configurable: true, get: d.get, set(v) { t.push('<img crossOrigin=' + v + '>'); d.set.call(this, v); } });
     return true;`,
  );
  const how = await goView('5', 'Skins');
  await waitFor(() => page.byCss('.viewer'), { timeout: 20000, what: '.viewer' });
  const mode = await until(
    () => page.exec(`const v = document.querySelector('.viewer'); if (!v) return null; if (document.querySelector('.serr')) return 'error'; if (v.querySelector('canvas.gl.ready')) return 'gl'; if (v.classList.contains('flat') && v.querySelector('.figures')) return 'flat'; return null;`),
    { timeout: 40000, what: 'the skin viewer to show the skin (gl or flat)' },
  );
  await sleep(2500); // let the model render a few frames
  const info = await page.exec(
    `const gl = document.createElement('canvas'); const ctx = gl.getContext('webgl2') || gl.getContext('webgl');
     const c = document.querySelector('.viewer canvas.gl');
     return { webgl: !!ctx, tex: window.__e2e.tex, glSize: c ? [c.width, c.height] : null, flatNote: !!document.querySelector('.flatnote'), pose: !!document.querySelector('[role=radiogroup], .bar') };`,
  );
  // in-page read-back attempt (informational; without preserveDrawingBuffer it can legitimately be blank)
  const rb = await page.s.execAsync(
    `const done = arguments[arguments.length - 1]; const c = document.querySelector('.viewer canvas.gl'); if (!c) return done(null);
     requestAnimationFrame(() => { try { const o = document.createElement('canvas'); o.width = Math.min(c.width, 200); o.height = Math.min(c.height, 200); const x = o.getContext('2d'); x.drawImage(c, 0, 0, o.width, o.height);
       const d = x.getImageData(0, 0, o.width, o.height).data; const s = new Set(); for (let i = 0; i < d.length; i += 4) s.add((d[i] >> 4) + ',' + (d[i + 1] >> 4) + ',' + (d[i + 2] >> 4) + ',' + (d[i + 3] >> 4)); done({ distinct: s.size }); } catch (e) { done({ err: String(e) }); } });`,
  );
  const { img, file } = await shotImg('s10-skins-3d');
  let verdict;
  if (mode === 'gl') {
    const r = await rectOf('.viewer canvas.gl');
    const sc = img.w / r.vw;
    const st = regionStats(img, { x: r.x * sc, y: r.y * sc, w: r.w * sc, h: r.h * sc });
    check(st.otherShare > 0.03 && st.significant >= 5, `the WebGL canvas looks empty/uniform in the screenshot (dominant ${st.dominant}, ${(st.otherShare * 100).toFixed(1)} % other pixels, ${st.significant} significant colours); see ${file}`);
    verdict = `WebGL model rendered: canvas ${Math.round(r.w)}x${Math.round(r.h)}, ${(st.otherShare * 100).toFixed(0)} % non-background pixels, ${st.significant} significant colours (bg ${st.dominant})`;
  } else if (mode === 'flat') {
    const r = await rectOf('.viewer .figures canvas.fig');
    const sc = img.w / r.vw;
    const st = regionStats(img, { x: r.x * sc, y: r.y * sc, w: r.w * sc, h: r.h * sc });
    check(st.significant >= 3, `flat fallback canvas is blank (${st.significant} colours)`);
    verdict = `flat fallback shown (${info.webgl ? 'WebGL IS available but the viewer fell back: texture refused (tainted/plain path) -> see Notes' : 'WebGL unavailable in this webview'}); figure ${st.significant} colours`;
    if (info.webgl) findings.push('S10: WebGL is available in this webview but the skin viewer fell back to the flat view (texture refused as tainted, or the renderer lost its context).');
  } else throw new Error(`skin viewer ended in state ${mode}`);
  const c = await page.collected();
  check(c.errors.length === 0, `page errors on the Skins page: ${c.errors.join(' | ')}`);
  return `${verdict}; opened via ${how}; texture path spy: ${(info.tex || []).join(', ') || 'no fetch/createImageBitmap/crossOrigin seen (cached or plain <img>)'}; in-page readback ${JSON.stringify(rb)}; ${file}`;
}

async function s11() {
  await ensureUi();
  const wid = x11.mainWindows(appPid, true)[0];
  check(wid, 'no X11 window for the app');
  const dom = await page.exec(
    `const q = (s) => [...document.querySelectorAll(s)];
     const r = (e) => { const b = e.getBoundingClientRect(); return { x: b.x, y: b.y, w: b.width, h: b.height }; };
     const strip = document.querySelector('header.strip'); if (!strip) return { missing: 'header.strip' };
     const brand = strip.querySelector('.brandbtn'); const arrows = q('header.strip .arrows .ab'); const wc = q('header.strip .wc .c');
     return { strip: r(strip), mode: strip.dataset.mode, brand: brand ? { ...r(brand), label: brand.getAttribute('aria-label'), hasArt: !!brand.querySelector('svg, img, canvas') || brand.textContent.trim().length > 0 } : null,
       arrows: arrows.map((a) => ({ label: a.getAttribute('aria-label'), disabled: a.disabled, ...r(a) })), wc: wc.map((a) => ({ label: a.getAttribute('aria-label'), ...r(a) })), rz: q('.rz').length, vw: innerWidth };`,
  );
  check(!dom.missing, `title bar element missing: ${dom.missing}`);
  check(dom.mode === 'linux', `title bar mode is ${dom.mode}, expected "linux" (custom controls)`);
  check(dom.brand && dom.brand.hasArt && dom.brand.w > 10, 'brand button missing or empty');
  check(dom.arrows.length === 2, `expected back/forward buttons, found ${dom.arrows.length}`);
  check(dom.wc.length === 3, `expected 3 window controls, found ${dom.wc.length}`);
  check(dom.rz === 8, `expected 8 resize zones, found ${dom.rz}`);
  check(Math.abs(dom.strip.h - 36) < 1.5, `title bar height ${dom.strip.h}, expected 36`);
  check(Math.abs(dom.wc[2].x + dom.wc[2].w - dom.vw) < 2, `close button is not flush with the right edge (${dom.wc[2].x + dom.wc[2].w} vs ${dom.vw})`);
  await page.shot('s11-titlebar');
  const ctrl = (i) => page.exec(`const c = document.querySelectorAll('header.strip .wc .c')[arguments[0]]; return c ? { label: c.getAttribute('aria-label'), svg: c.innerHTML.length } : null;`, [i]);
  const clickCtrl = async (i) => {
    const el = await page.exec(`return document.querySelectorAll('header.strip .wc .c')[arguments[0]] || null;`, [i]);
    await session.click(Object.values(el)[0]);
  };
  const isMax = async () => (await page.invoke('window_is_maximized')).ok === true;
  const rz = () => page.exec(`return document.querySelectorAll('.rz').length;`);
  const g0 = rectInfo(wid);
  const maxBefore = await ctrl(1);
  const notes = [];
  // maximize / restore through the buttons
  await clickCtrl(1);
  await until(isMax, { timeout: 6000, what: 'window to report maximized' });
  await sleep(500);
  const g1 = rectInfo(wid);
  const maxAfter = await until(async () => { const c = await ctrl(1); return c && c.label !== maxBefore.label ? c : null; }, { timeout: 5000, what: 'maximize icon to swap to restore (window:state)' });
  check(g1.w > g0.w || g1.h > g0.h, `X11 geometry did not grow on maximize (${g0.w}x${g0.h} -> ${g1.w}x${g1.h}; screen may equal the window)`);
  check((await rz()) === 0, 'resize zones still present while maximized');
  await page.shot('s11-maximized');
  notes.push(`maximize ${g0.w}x${g0.h} -> ${g1.w}x${g1.h}, button "${maxBefore.label}" -> "${maxAfter.label}", resize zones 8 -> 0`);
  await clickCtrl(1);
  await until(async () => !(await isMax()), { timeout: 6000, what: 'window to report restored' });
  await sleep(500);
  const g2 = rectInfo(wid);
  check(Math.abs(g2.w - g0.w) < 40 && Math.abs(g2.h - g0.h) < 40, `restore did not return to the old size (${g0.w}x${g0.h} vs ${g2.w}x${g2.h})`);
  await until(async () => (await rz()) === 8, { timeout: 4000, what: '8 resize zones after restore' });
  await until(async () => { const c = await ctrl(1); return c && c.label === maxBefore.label; }, { timeout: 4000, what: 'restore icon to swap back to maximize' });
  notes.push(`restore -> ${g2.w}x${g2.h}, zones back to 8`);
  // double click on the drag region (real X11 clicks)
  const mid = await rectOf('header.strip .mid');
  const gw = rectInfo(wid);
  x11.focus(wid);
  x11.doubleClick(gw.x + mid.x + mid.w / 2, gw.y + mid.y + mid.h / 2);
  const dbl1 = await until(isMax, { timeout: 4000, what: 'double click on the drag region to maximize' }).then(() => true, () => false);
  if (dbl1) {
    await sleep(600);
    const gm = await rectOf('header.strip .mid');
    const gg = rectInfo(wid);
    x11.doubleClick(gg.x + gm.x + gm.w / 2, gg.y + gm.y + gm.h / 2);
    const back = await until(async () => !(await isMax()), { timeout: 4000, what: 'second double click to restore' }).then(() => true, () => false);
    check(back, 'double click on the drag region maximized but a second one did not restore');
    notes.push('double click on the strip toggles maximize both ways');
    await sleep(500);
  } else {
    // may be an xdotool/WM timing issue rather than an app bug: say so
    throw new Error(`double click on the drag region did not maximize (xdotool at ${Math.round(gw.x + mid.x + mid.w / 2)},${Math.round(gw.y + mid.y + mid.h / 2)}); if the WM swallowed the clicks verify by hand`);
  }
  // drag / resize commands (the WM grabs the pointer; release it afterwards)
  for (const [cmd, args] of [['window_start_drag', {}], ['window_start_resize', { direction: 'east' }], ['window_start_resize', { direction: 'southwest' }]]) {
    const r = await page.invoke(cmd, args);
    check(!r.err, `${cmd} ${JSON.stringify(args)} errored: ${r.err}`);
    x11.key('Escape');
    x11.mouseUp();
    await sleep(400);
  }
  notes.push('window_start_drag / window_start_resize return ok');
  // minimize
  await clickCtrl(0);
  const unmapped = await until(() => x11.mainWindows(appPid, true).length === 0, { timeout: 5000, what: 'window to be unmapped after Minimize' }).then(() => true, () => false);
  check(unmapped, 'Minimize did not unmap/iconify the window');
  x11.mapWindow(wid);
  x11.activate(wid);
  const back = await until(() => x11.mainWindows(appPid, true).length > 0, { timeout: 6000, what: 'window to come back (windowmap/activate)' }).then(() => true, () => false);
  if (!back) findings.push('S11: after Minimize the window could not be restored with xdotool windowmap/windowactivate; check by hand that the taskbar restores it.');
  notes.push(`minimize unmapped the window; restored via xdotool: ${back}`);
  await sleep(600);
  await page.shot('s11-after-minimize-restore');
  const c = await page.collected();
  check(c.errors.length === 0, `page errors during the title bar test: ${c.errors.join(' | ')}`);
  return notes.join('; ');
}

/** Give the app window real keyboard focus (cycling and other focus-gated UI pause otherwise) and verify it with document.hasFocus(). */
async function focusWindow() {
  const wid = x11.mainWindows(appPid, true)[0];
  check(wid, 'no visible window to focus');
  const has = () => page.exec('return document.hasFocus();').catch(() => false);
  for (let attempt = 0; attempt < 4; attempt++) {
    x11.activate(wid);
    x11.focus(wid);
    await sleep(700);
    if (await has()) return `focused (attempt ${attempt + 1})`;
    if (attempt >= 1) {
      // some compositors refuse focus requests from xdotool: a real click on the title bar area does it (harmless: no control there)
      const g = x11.geometry(wid);
      spawnSync('xdotool', ['mousemove', String(Math.round(g.x + g.w / 2)), String(Math.round(g.y + 6)), 'click', '1']);
      await sleep(700);
      if (await has()) return `focused by click (attempt ${attempt + 1})`;
    }
  }
  throw new Error('the app window never got focus (document.hasFocus() stays false after windowactivate/windowfocus and a title bar click)');
}

async function s12() {
  await ensureUi();
  const focusNote = await focusWindow();
  await goView('1', 'Home');
  await waitFor(() => page.byCss('.hero'), { timeout: 15000, what: '.hero on Home' });
  const sample = () => page.exec(
    `const st = document.querySelector('.hero .stack');
     if (!st) return { stack: false, focus: document.hasFocus(), hidden: document.hidden, rm: matchMedia('(prefers-reduced-motion: reduce)').matches };
     const slots = [...st.querySelectorAll('.slot')].map((s) => { const i = s.querySelector('img'); const b = s.querySelector('[style*="background"]'); return { cls: s.className.replace(/svelte-\\w+/g, '').trim(), key: i ? i.getAttribute('src') : b ? b.getAttribute('style').slice(0, 80) : s.innerHTML.length + ':' + s.innerHTML.slice(0, 60) }; });
     return { stack: true, focus: document.hasFocus(), hidden: document.hidden, slots };`,
  );
  await sleep(4000);
  const first = await sample();
  const pool = (await page.invoke('panorama_pool')).ok || [];
  await page.shot('s12-home-start');
  if (!first.stack) {
    // not cycling: legitimate only with a single pool member (or the setting off / a fixed background)
    const pref = await page.exec(`try { return localStorage.getItem('mango.cycleBackgrounds'); } catch { return 'n/a'; }`);
    if (pool.length >= 1) throw new Error(`Home is not cycling although ${pool.length} panorama set(s) exist (cycle pref ${pref}, focus ${first.focus}, hidden ${first.hidden}); instance background may be fixed rather than "auto"`);
    return `no cycling: pool has no panorama sets and there is no stack layer (brand + wallpapers only, or a single member); nothing to swap (not cycling is expected)`;
  }
  const keys = (s) => (s.slots || []).map((x) => x.key).sort().join('|');
  const seen = new Set([keys(first)]);
  const t0c = Date.now();
  let changes = 0;
  let last = keys(first);
  let paused = 0;
  // every unfocused/hidden period re-arms the cycler with a fresh 30-60 s delay (bgCycle.ts), so allow extra time after pauses
  while (Date.now() - t0c < 75000 + Math.min(paused, 3) * 40000) {
    await sleep(1000);
    const s = await sample();
    if (!s.focus || s.hidden) { paused++; if (paused <= 3 || paused % 10 === 0) await focusWindow().catch(() => {}); } // regain focus: the cycler pauses while unfocused
    const k = keys(s);
    if (k !== last) { changes++; seen.add(k); last = k; if (changes === 1) await page.shot('s12-home-after-swap'); if (changes >= 2) break; }
  }
  const secs = Math.round((Date.now() - t0c) / 1000);
  check(changes >= 1, `the Home hero background did not change within ${secs} s (focus lost in ${paused} samples, reduced-motion ${await page.exec("return matchMedia('(prefers-reduced-motion: reduce)').matches;").catch(() => '?')}; cycling pauses while the window is unfocused or the document hidden); pool sets ${pool.length}`);
  return `${focusNote}; hero swapped ${changes}x within ${secs} s (${seen.size} distinct layer states, ${pool.length} panorama set(s) in the pool, unfocused samples ${paused})`;
}

async function s13() {
  await ensureUi();
  const wid = x11.mainWindows(appPid, true)[0];
  if (wid) x11.activate(wid);
  const shots = [];
  const survey = async (name) => {
    await sleep(1200);
    const f = await page.shot(name);
    const broken = await page.exec(`return [...document.images].filter((i) => i.complete && i.naturalWidth === 0 && i.src).map((i) => i.src.slice(0, 70));`);
    const overflow = await page.exec(`return document.documentElement.scrollWidth > innerWidth + 1;`);
    shots.push(`${name}${broken.length ? ` (${broken.length} broken <img>)` : ''}${overflow ? ' (horizontal overflow)' : ''}`);
    if (broken.length) findings.push(`S13 ${name}: broken images ${broken.slice(0, 3).join(', ')}`);
    return f;
  };
  await goView('1', 'Home'); await survey('s13-01-home');
  await goView('2', 'Library'); await survey('s13-02-library');
  await goView('5', 'Skins'); await survey('s13-03-skins');
  await combo(','); await survey('s13-04-settings');
  // an instance page with every tab
  await page.click(/^Vanilla Test$/, { selector: 'nav.rail button.prof' });
  await sleep(400);
  // a single click only selects an instance now; Enter on the focused tile opens it
  await session.keys(['\uE007']);
  await sleep(800);
  const tabs = await page.exec(`return [...document.querySelectorAll('[role=tablist] [role=tab]')].map((t) => t.textContent.trim());`);
  check(tabs.length >= 5, `instance page shows only ${tabs.length} tabs`);
  for (const [i, label] of tabs.entries()) {
    const id = await page.findText(new RegExp(`^${label.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}$`), '[role=tab]');
    if (id) { await session.click(id); await sleep(600); }
    await survey(`s13-05-instance-${String(i + 1).padStart(2, '0')}-${label.toLowerCase().replace(/[^a-z0-9]+/g, '-')}`);
  }
  // icon picker
  let cells = 0;
  const open = await page.findText(/Change icon/i, 'button.iconbtn, button');
  if (open) {
    await session.click(open);
    await waitFor(() => page.byCss('dialog[open]'), { timeout: 8000, what: 'icon picker dialog' });
    cells = await until(() => page.exec(`return document.querySelectorAll('dialog[open] [role=radio].cell').length;`), { timeout: 60000, interval: 500, what: 'block icons in the picker gallery' }).catch(() => 0);
    await sleep(1500);
    const imgs = await page.exec(`const im = [...document.querySelectorAll('dialog[open] .cell img')]; return { n: im.length, ok: im.filter((i) => i.complete && i.naturalWidth > 0).length };`);
    await survey('s13-06-icon-picker');
    shots.push(`icon picker: ${cells} cells, ${imgs.ok}/${imgs.n} face images decoded`);
    await page.exec(`document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true })); return true;`);
    await sleep(500);
    if (await page.byCss('dialog[open]')) { await session.keys(['']).catch(() => {}); await sleep(500); }
  } else findings.push('S13: no "Change icon" button found on the instance page');
  // shortcut sheet
  await combo('/'); await sleep(500);
  let sheet = await page.byCss('dialog[open], [role=dialog]');
  let via = 'Ctrl+/';
  if (!sheet) { await session.keys(['\uE031']); await sleep(500); sheet = await page.byCss('dialog[open], [role=dialog]'); via = sheet ? 'F1 (Ctrl+/ did not)' : via; }
  await survey('s13-07-shortcuts');
  if (via !== 'Ctrl+/') findings.push(`S13: the shortcut sheet opened via ${via}`);
  check(!!sheet, 'neither Ctrl+/ nor F1 opened the shortcut sheet');
  await session.keys(['']).catch(() => {});
  const c = await page.collected();
  check(c.errors.length === 0, `page errors during the survey: ${c.errors.join(' | ')}`);
  return `${shots.length} screenshots in ${OUT}: ${shots.join(', ')}`;
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
  for (const p of pids()) {
    if (exeOf(p) !== BINARY) continue;
    const h = envOf(p).MANGOCLIENT_HOME;
    if (h === HOME_DIR || COPIES.includes(h)) {
      for (const j of x11.javaChildren(p)) kill(j, 'SIGKILL');
      kill(p, 'SIGKILL');
    }
  }
  if (driverPid) kill(driverPid, 'SIGTERM');
  if (!KEEP) {
    // data dir only; screenshots and logs stay
    rmSync(HOME_DIR, { recursive: true, force: true });
    for (const c of COPIES) rmSync(c, { recursive: true, force: true }); // the copies of the prepared data dir
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
  process.env.MANGOCLIENT_HOME = HOME_DIR; // inherited by the driver; the wrappers of S9+ export their own
  if (!argv.includes('--native')) {
    // XWayland: lets S8 close the window like the WM close button. Software-ish buffers: the dmabuf path fails on XWayland here.
    process.env.GDK_BACKEND = 'x11';
    process.env.WEBKIT_DISABLE_DMABUF_RENDERER = '1';
  }
  process.env.TAURI_WEBVIEW_AUTOMATION = 'true'; // tauri-runtime-wry: allow WebKit automation (what tauri-driver sets)
  await startDriver();
  const mainWanted = [...'12345678'].some((n) => wanted(`S${n}`));
  if (mainWanted) {
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
    if (session) await session.quit();
    session = null;
    page = null;
  }

  // S9..S13 run on copies of the prepared data dir, one app instance per lifecycle scenario
  const fresh = async (fn) => { try { return await fn(); } finally { await endApp(); } };
  await scenario('S9a', 'hideOnLaunch: window destroyed while the game runs, re-created at game end', () => fresh(s9a));
  await scenario('S9b', 'hideOnLaunch: second start restores the window; closing it during a game does not quit', () => fresh(s9b));
  const uiWanted = ['S10', 'S11', 'S12', 'S13'].some(wanted);
  if (uiWanted) {
    await scenario('S10', '3D skin viewer renders the skin in the real shell', s10);
    await scenario('S11', 'frameless title bar: controls, maximize, minimize, resize zones', s11);
    await scenario('S13', 'visual survey screenshots (every view, tabs, icon picker, shortcuts)', s13);
    await scenario('S12', 'Home background cycling (up to 75 s)', s12);
    await endApp();
  }
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
