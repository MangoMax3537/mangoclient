'use strict';
const fs = require('fs');
const path = require('path');
const { containsPath } = require('./security');

const LIMITS = Object.freeze({
  index: 4 * 1024 * 1024,
  entries: 50_000,
  overrideEntry: 256 * 1024 * 1024,
  overridesTotal: 2 * 1024 * 1024 * 1024,
  metadata: 1024 * 1024,
});

function safeArchivePath(root, relative) {
  if (typeof relative !== 'string' || relative.includes('\0') || path.isAbsolute(relative)) {
    throw new Error('Archive contains an invalid path');
  }
  const target = path.resolve(root, relative);
  if (!containsPath(path.resolve(root), target)) throw new Error(`Archive path escapes its instance: ${relative}`);

  // Refuse an existing symlink at any level. This closes the gap where a
  // previous install (or a local user) points an innocent-looking folder out.
  let current = path.resolve(root);
  for (const part of path.relative(current, target).split(path.sep)) {
    current = path.join(current, part);
    try {
      if (fs.lstatSync(current).isSymbolicLink()) throw new Error(`Archive path uses a symlink: ${relative}`);
    } catch (err) {
      if (err.code === 'ENOENT') break;
      throw err;
    }
  }
  return target;
}

function isZipSymlink(entry) {
  const mode = (Number(entry?.attr) >>> 16) & 0o170000;
  return mode === 0o120000;
}

function validateArchiveEntries(entries) {
  if (!Array.isArray(entries) || entries.length > LIMITS.entries) throw new Error('Archive has too many entries');
  let total = 0;
  for (const entry of entries) {
    if (isZipSymlink(entry)) throw new Error(`Archive contains a symlink: ${entry.entryName}`);
    if (entry.isDirectory) continue;
    const size = Number(entry.header?.size);
    if (!Number.isSafeInteger(size) || size < 0 || size > LIMITS.overrideEntry) {
      throw new Error(`Archive entry is too large: ${entry.entryName}`);
    }
    total += size;
    if (total > LIMITS.overridesTotal) throw new Error('Archive overrides are too large');
  }
  return total;
}

module.exports = { LIMITS, safeArchivePath, isZipSymlink, validateArchiveEntries };
