'use strict';
const fs = require('fs');
const path = require('path');
const crypto = require('crypto');
const { app } = require('electron');
const P = require('./paths');
const { validatedApiBase } = require('./security');

/**
 * The two numbers on mangoclient's website: how many copies exist, and how
 * many are open right now.
 *
 * What leaves the machine is a random id, the launcher version and the
 * platform string. No account, no name, no hardware, nothing that says who is
 * behind the id - and the id lives inside the launcher's own data root, so
 * deleting MangoClient forgets it too.
 */

let BASE;
try {
  BASE = validatedApiBase(process.env.MANGO_API_BASE);
} catch (err) {
  console.warn(`[telemetry] ${err.message}; using the legacy endpoint`);
  BASE = validatedApiBase();
}
const FILE = path.join(P.root, 'install.json');
const BEAT_MS = 60 * 1000;
const TIMEOUT_MS = 8000;

let timer = null;

/** The id for this copy, made once and kept. */
function installId() {
  try {
    const saved = JSON.parse(fs.readFileSync(FILE, 'utf8'));
    if (typeof saved.id === 'string' && saved.id.length >= 8) return saved.id;
  } catch {
    // No file yet, or someone edited it into nonsense: make a new one.
  }
  const id = crypto.randomUUID();
  try {
    fs.mkdirSync(path.dirname(FILE), { recursive: true });
    fs.writeFileSync(FILE, JSON.stringify({ id, created: new Date().toISOString() }, null, 2));
  } catch {
    // A read-only data root should not stop the launcher from starting; the
    // id then changes per run, which only inflates nothing - the server keys
    // installs by id, and this copy simply counts as new each time.
  }
  return id;
}

async function beat(id) {
  const controller = new AbortController();
  const cancel = setTimeout(() => controller.abort(), TIMEOUT_MS);
  try {
    await fetch(`${BASE}/v1/launcher`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ id, version: app.getVersion(), os: process.platform }),
      signal: controller.signal,
    });
  } catch {
    // Offline, or the server is down. Nothing here is worth a log line.
  } finally {
    clearTimeout(cancel);
  }
}

/**
 * Start beating. `isEnabled` is asked before every beat rather than once, so
 * turning the setting off stops the next one instead of needing a restart.
 */
function start(isEnabled = () => true) {
  if (timer) return;
  const id = installId();
  const tick = () => { if (isEnabled()) beat(id); };
  tick();
  timer = setInterval(tick, BEAT_MS);
  timer.unref?.();
}

function stop() {
  if (timer) clearInterval(timer);
  timer = null;
}

module.exports = { start, stop, installId };
