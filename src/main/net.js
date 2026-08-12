'use strict';
const fs = require('fs');
const fsp = require('fs/promises');
const path = require('path');
const crypto = require('crypto');
const { Readable, Transform } = require('stream');
const { pipeline } = require('stream/promises');

const UA = 'MangoClient/1.0.0 (+https://github.com/mangomax/mangoclient)';

async function fetchWithRetry(url, opts = {}, retries = 4) {
  let lastErr;
  for (let attempt = 0; attempt <= retries; attempt++) {
    try {
      const ctrl = new AbortController();
      const timeout = setTimeout(() => ctrl.abort(), opts.timeout || 60000);
      try {
        const res = await fetch(url, {
          ...opts,
          signal: ctrl.signal,
          headers: { 'User-Agent': UA, ...(opts.headers || {}) },
        });
        // 5xx and 429 are worth retrying; other non-OK codes are not.
        if (!res.ok && (res.status >= 500 || res.status === 429)) {
          throw new Error(`HTTP ${res.status} for ${url}`);
        }
        return res;
      } finally {
        clearTimeout(timeout);
      }
    } catch (err) {
      lastErr = err;
      if (attempt === retries) break;
      await new Promise((r) => setTimeout(r, 400 * Math.pow(2, attempt)));
    }
  }
  throw lastErr;
}

async function getJSON(url, opts = {}) {
  const res = await fetchWithRetry(url, opts);
  if (!res.ok) throw new Error(`HTTP ${res.status} for ${url}`);
  return res.json();
}

async function getBuffer(url, opts = {}) {
  const res = await fetchWithRetry(url, opts);
  if (!res.ok) throw new Error(`HTTP ${res.status} for ${url}`);
  return Buffer.from(await res.arrayBuffer());
}

function hashFile(file, algo = 'sha1') {
  return new Promise((resolve, reject) => {
    const hash = crypto.createHash(algo);
    const stream = fs.createReadStream(file);
    stream.on('error', reject);
    stream.on('data', (chunk) => hash.update(chunk));
    stream.on('end', () => resolve(hash.digest('hex')));
  });
}

const sha1File = (file) => hashFile(file, 'sha1');

async function isValid(file, sha1, size, sha256) {
  try {
    const stat = await fsp.stat(file);
    if (size != null && stat.size !== size) return false;
    if (sha256) return (await hashFile(file, 'sha256')) === sha256.toLowerCase();
    if (!sha1) return stat.size > 0;
    return (await sha1File(file)) === sha1;
  } catch {
    return false;
  }
}

/**
 * Download a file to `dest`, skipping the transfer when a valid copy already
 * exists. Writes through a temp file so an interrupted run never leaves a
 * truncated jar behind that would later be treated as complete.
 */
async function downloadFile(url, dest, { sha1, sha256, size, onBytes } = {}) {
  if (await isValid(dest, sha1, size, sha256)) {
    if (onBytes && size) onBytes(size);
    return { skipped: true, dest };
  }
  await fsp.mkdir(path.dirname(dest), { recursive: true });
  const tmp = `${dest}.part-${process.pid}-${Math.floor(performance.now() * 1000) % 1e6}`;
  const res = await fetchWithRetry(url);
  if (!res.ok) throw new Error(`HTTP ${res.status} for ${url}`);

  let counted = 0;
  // Progress is counted by a Transform *inside* the pipeline. Attaching a
  // plain 'data' listener would switch the stream to flowing mode before the
  // destination is wired up, which silently corrupts the file.
  const meter = new Transform({
    transform(chunk, _enc, cb) {
      counted += chunk.length;
      onBytes?.(chunk.length);
      cb(null, chunk);
    },
  });

  await pipeline(Readable.fromWeb(res.body), meter, fs.createWriteStream(tmp));

  const verify = async (algo, expected) => {
    if (!expected) return;
    const actual = await hashFile(tmp, algo);
    if (actual !== expected.toLowerCase()) {
      await fsp.unlink(tmp).catch(() => {});
      throw new Error(`Checksum mismatch for ${url}: expected ${expected}, got ${actual}`);
    }
  };
  await verify('sha1', sha1);
  await verify('sha256', sha256);

  if (size != null && !sha1 && !sha256) {
    const stat = await fsp.stat(tmp);
    if (stat.size !== size) {
      await fsp.unlink(tmp).catch(() => {});
      throw new Error(`Short read for ${url}: expected ${size} bytes, got ${stat.size}`);
    }
  }

  await fsp.rename(tmp, dest);
  return { skipped: false, dest, bytes: counted };
}

/** Run async tasks with bounded concurrency, preserving result order. */
async function pool(items, limit, worker) {
  const results = new Array(items.length);
  let cursor = 0;
  const runners = Array.from({ length: Math.min(limit, items.length) }, async () => {
    while (true) {
      const i = cursor++;
      if (i >= items.length) return;
      results[i] = await worker(items[i], i);
    }
  });
  await Promise.all(runners);
  return results;
}

module.exports = { fetchWithRetry, getJSON, getBuffer, downloadFile, sha1File, hashFile, isValid, pool, UA };
