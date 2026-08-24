'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs/promises');
const os = require('node:os');
const path = require('node:path');
const { LIMITS, safeArchivePath, validateArchiveEntries } = require('../src/main/archive');

test('archive target paths cannot traverse or follow existing symlinks', async (t) => {
  const root = await fs.mkdtemp(path.join(os.tmpdir(), 'mango-archive-'));
  t.after(() => fs.rm(root, { recursive: true, force: true }));
  assert.equal(safeArchivePath(root, 'mods/safe.jar'), path.join(root, 'mods', 'safe.jar'));
  assert.throws(() => safeArchivePath(root, '../escape.jar'), /escapes/);
  const outside = await fs.mkdtemp(path.join(os.tmpdir(), 'mango-outside-'));
  t.after(() => fs.rm(outside, { recursive: true, force: true }));
  await fs.symlink(outside, path.join(root, 'linked'));
  assert.throws(() => safeArchivePath(root, 'linked/escape.jar'), /symlink/);
});

test('archive limits cap entry count, individual and total override sizes', () => {
  const entry = (name, size, attr = 0) => ({ entryName: name, isDirectory: false, header: { size }, attr });
  assert.equal(validateArchiveEntries([entry('a', 10), entry('b', 20)]), 30);
  assert.throws(() => validateArchiveEntries(new Array(LIMITS.entries + 1)), /too many/);
  assert.throws(() => validateArchiveEntries([entry('huge', LIMITS.overrideEntry + 1)]), /too large/);
  assert.throws(() => validateArchiveEntries([entry('link', 1, 0o120000 << 16)]), /symlink/);
  assert.throws(() => validateArchiveEntries(Array.from({ length: 9 }, (_, i) => entry(String(i), LIMITS.overrideEntry))), /overrides/);
});
