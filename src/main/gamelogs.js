'use strict';
const fsp = require('fs/promises');
const fs = require('fs');
const path = require('path');
const zlib = require('zlib');
const { Writable } = require('stream');
const { pipeline } = require('stream/promises');
const { shell } = require('electron');
const P = require('./paths');
const { fetchWithRetry } = require('./net');

/**
 * Log files belonging to one instance, the way OneLauncher's log viewer sees
 * them: what the game wrote, what it crashed with, and what the launcher
 * itself captured for the last run.
 *
 * Reading is deliberately read-only and capped - a log the game is still
 * appending to can be arbitrarily large, and nobody scrolls 400 MB.
 */

/** A log longer than this is shown from its end; that is where the error is. */
const MAX_BYTES = 4 * 1024 * 1024;
/** mclo.gs rejects pastes past its own limits, so trim before uploading. */
const UPLOAD_MAX_LINES = 25000;
const MCLOGS_API = 'https://api.mclo.gs/1/log';

const KINDS = {
  game: (profileId) => path.join(P.instanceDir(profileId), 'logs'),
  crash: (profileId) => path.join(P.instanceDir(profileId), 'crash-reports'),
};

const LOG_RE = /\.(log|log\.gz|txt)$/i;

/** `game:latest.log` -> {kind, name}; anything else is rejected. */
function parseId(id) {
  const m = /^(game|crash|launcher):(.+)$/.exec(String(id || ''));
  if (!m) throw new Error('Unknown log');
  const name = path.basename(m[2]);
  if (!name || name === '.' || name === '..') throw new Error('Unknown log');
  return { kind: m[1], name };
}

function fileForId(profileId, id, launcherLog) {
  const { kind, name } = parseId(id);
  if (kind === 'launcher') {
    if (!launcherLog || path.basename(launcherLog) !== name) throw new Error('Unknown log');
    return launcherLog;
  }
  return path.join(KINDS[kind](profileId), name);
}

async function listDir(dir, kind) {
  let names;
  try {
    names = await fsp.readdir(dir);
  } catch {
    return [];
  }
  const out = [];
  for (const name of names) {
    if (!LOG_RE.test(name)) continue;
    try {
      const stat = await fsp.stat(path.join(dir, name));
      if (!stat.isFile()) continue;
      out.push({ id: `${kind}:${name}`, kind, name, size: stat.size, modified: stat.mtimeMs });
    } catch { /* rotated away while we listed */ }
  }
  return out;
}

/**
 * Newest first, but `latest.log` always leads: it is the run the player just
 * had, and its mtime can lag behind a freshly rotated archive.
 */
async function listLogs(profileId, launcherLog) {
  const entries = [
    ...(await listDir(KINDS.game(profileId), 'game')),
    ...(await listDir(KINDS.crash(profileId), 'crash')),
  ];

  if (launcherLog) {
    try {
      const stat = await fsp.stat(launcherLog);
      entries.push({
        id: `launcher:${path.basename(launcherLog)}`,
        kind: 'launcher',
        name: path.basename(launcherLog),
        size: stat.size,
        modified: stat.mtimeMs,
      });
    } catch { /* the instance has never been launched */ }
  }

  entries.sort((a, b) => b.modified - a.modified);
  const latest = entries.findIndex((e) => e.kind === 'game' && e.name === 'latest.log');
  if (latest > 0) entries.unshift(...entries.splice(latest, 1));
  return entries;
}

async function readTail(file) {
  const stat = await fsp.stat(file);
  if (!file.endsWith('.gz')) {
    const length = Math.min(stat.size, MAX_BYTES);
    const handle = await fsp.open(file, 'r');
    try {
      const data = Buffer.allocUnsafe(length);
      const { bytesRead } = await handle.read(data, 0, length, stat.size - length);
      return { text: data.subarray(0, bytesRead).toString('utf8'), bytes: stat.size, truncated: stat.size > length };
    } finally {
      await handle.close();
    }
  }

  const chunks = [];
  let kept = 0;
  let total = 0;
  const tail = new Writable({
    write(chunk, _encoding, callback) {
      total += chunk.length;
      chunks.push(chunk);
      kept += chunk.length;
      while (kept > MAX_BYTES && chunks.length) {
        const excess = kept - MAX_BYTES;
        if (chunks[0].length <= excess) kept -= chunks.shift().length;
        else {
          chunks[0] = chunks[0].subarray(excess);
          kept -= excess;
        }
      }
      callback();
    },
  });
  await pipeline(fs.createReadStream(file), zlib.createGunzip(), tail);
  return { text: Buffer.concat(chunks, kept).toString('utf8'), bytes: total, truncated: total > kept };
}

/**
 * Read a log for display. Returns the tail when the file is too big, and says
 * so, rather than pretending the beginning was never there.
 */
async function readLog(profileId, id, launcherLog) {
  const file = fileForId(profileId, id, launcherLog);
  const { text, bytes, truncated } = await readTail(file);
  return {
    id,
    name: path.basename(file),
    text,
    truncated,
    bytes,
  };
}

/**
 * Share a log through mclo.gs, the paste service the Minecraft community uses
 * for support threads. Returns the page URL, which the caller shows and the
 * player pastes wherever they are asking for help.
 */
async function uploadLog(profileId, id, launcherLog) {
  const { text } = await readLog(profileId, id, launcherLog);
  const lines = text.split(/\r?\n/);
  const body = lines.length > UPLOAD_MAX_LINES ? lines.slice(-UPLOAD_MAX_LINES).join('\n') : text;

  const res = await fetchWithRetry(MCLOGS_API, {
    method: 'POST',
    headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
    body: new URLSearchParams({ content: body }).toString(),
  });
  const data = await res.json().catch(() => null);
  if (!res.ok || !data?.success) {
    throw new Error(data?.error || `mclo.gs refused the upload (HTTP ${res.status})`);
  }
  return { url: data.url, id: data.id, raw: data.raw };
}

async function openLogsFolder(profileId) {
  const dir = KINDS.game(profileId);
  await fsp.mkdir(dir, { recursive: true });
  shell.openPath(dir);
  return dir;
}

async function deleteLog(profileId, id, launcherLog) {
  const file = fileForId(profileId, id, launcherLog);
  if (path.basename(file) === 'latest.log') throw new Error('The current log cannot be deleted');
  await fsp.unlink(file);
  return true;
}

module.exports = { listLogs, readLog, uploadLog, deleteLog, openLogsFolder };
