'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const fsp = require('node:fs/promises');
const os = require('node:os');
const path = require('node:path');
const { readJSON, writeJSONAtomic } = require('../src/main/persistence');

test('JSON saves retain a last-known-good generation and recover a truncated main file', async (t) => {
  const dir = await fsp.mkdtemp(path.join(os.tmpdir(), 'mango-persistence-'));
  t.after(() => fsp.rm(dir, { recursive: true, force: true }));
  const file = path.join(dir, 'accounts.json');

  writeJSONAtomic(file, { profiles: [{ id: 'first' }] });
  writeJSONAtomic(file, { profiles: [{ id: 'second' }] });
  fs.writeFileSync(file, '{"profiles":');

  assert.deepEqual(readJSON(file, { profiles: [] }), { profiles: [{ id: 'first' }] });
  assert.deepEqual(JSON.parse(fs.readFileSync(file, 'utf8')), { profiles: [{ id: 'first' }] });
});

test('first JSON save creates two independently readable copies', async (t) => {
  const dir = await fsp.mkdtemp(path.join(os.tmpdir(), 'mango-persistence-'));
  t.after(() => fsp.rm(dir, { recursive: true, force: true }));
  const file = path.join(dir, 'config.json');

  writeJSONAtomic(file, { language: 'de' });
  assert.deepEqual(JSON.parse(fs.readFileSync(file, 'utf8')), { language: 'de' });
  assert.deepEqual(JSON.parse(fs.readFileSync(`${file}.bak`, 'utf8')), { language: 'de' });
});
