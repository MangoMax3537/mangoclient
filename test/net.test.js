'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs/promises');
const os = require('node:os');
const path = require('node:path');
const crypto = require('node:crypto');
const { downloadFile } = require('../src/main/net');

test('downloads enforce SHA-512 and exact size and clean failed temporary files', async (t) => {
  const payload = Buffer.from('verified jar bytes');
  const originalFetch = global.fetch;
  global.fetch = async () => new Response(payload, { headers: { 'Content-Length': String(payload.length) } });
  t.after(() => { global.fetch = originalFetch; });
  const tmp = await fs.mkdtemp(path.join(os.tmpdir(), 'mango-download-'));
  t.after(() => fs.rm(tmp, { recursive: true, force: true }));
  const url = 'https://downloads.example/file.jar';
  const dest = path.join(tmp, 'file.jar');
  const sha512 = crypto.createHash('sha512').update(payload).digest('hex');
  await downloadFile(url, dest, { sha512, size: payload.length });
  assert.deepEqual(await fs.readFile(dest), payload);
  const bad = path.join(tmp, 'bad.jar');
  await assert.rejects(downloadFile(url, bad, { sha512: '0'.repeat(128), size: payload.length }), /Checksum/);
  assert.equal(await fs.stat(bad).then(() => true, () => false), false);
  assert.deepEqual((await fs.readdir(tmp)).filter((name) => name.includes('.part-')), []);
  await assert.rejects(downloadFile(url, path.join(tmp, 'wrong-size.jar'), { size: payload.length + 1 }), /Size mismatch/);
});

test('downloads ignore a CDN Content-Length placeholder of zero', async (t) => {
  const payload = Buffer.from('mojang metadata');
  const originalFetch = global.fetch;
  global.fetch = async () => new Response(payload, { headers: { 'Content-Length': '0' } });
  t.after(() => { global.fetch = originalFetch; });
  const tmp = await fs.mkdtemp(path.join(os.tmpdir(), 'mango-zero-length-'));
  t.after(() => fs.rm(tmp, { recursive: true, force: true }));
  const dest = path.join(tmp, 'version.json');
  await downloadFile('https://piston-meta.mojang.com/version.json', dest, { size: payload.length });
  assert.deepEqual(await fs.readFile(dest), payload);
});
