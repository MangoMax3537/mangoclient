'use strict';
const fsp = require('fs/promises');
const path = require('path');
const P = require('./paths');

/**
 * What the launcher's own folder is spending disk on, and which parts of it
 * are safe to throw away. Mirrors OneLauncher's storage settings: everything
 * listed here can be re-downloaded, except the instances, which hold worlds.
 */

/** Directories shown in the breakdown, in the order they are drawn. */
const SECTIONS = [
  { key: 'instances', dir: P.instances, removable: false },
  { key: 'versions', dir: P.versions, removable: true },
  { key: 'libraries', dir: P.libraries, removable: true },
  { key: 'assets', dir: P.assets, removable: true },
  { key: 'runtimes', dir: P.runtimes, removable: true },
  { key: 'natives', dir: P.natives, removable: true },
  { key: 'cache', dir: P.cache, removable: true },
  { key: 'logs', dir: P.logs, removable: true },
];

/**
 * Recursive size of a directory. Symlinks are counted by their own entry
 * rather than followed, so a link into the instances folder cannot make the
 * cache look enormous - or send the walk in circles.
 */
async function dirSize(dir) {
  let total = 0;
  let entries;
  try {
    entries = await fsp.readdir(dir, { withFileTypes: true });
  } catch {
    return 0; // never created, or removed under us
  }
  for (const entry of entries) {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      total += await dirSize(full);
    } else {
      try {
        total += (await fsp.lstat(full)).size;
      } catch { /* vanished mid-walk */ }
    }
  }
  return total;
}

async function usage() {
  const sections = [];
  for (const s of SECTIONS) {
    sections.push({ key: s.key, path: s.dir, bytes: await dirSize(s.dir), removable: s.removable });
  }
  return {
    root: P.root,
    total: sections.reduce((sum, s) => sum + s.bytes, 0),
    sections,
  };
}

/** Empty a directory without removing the directory itself. */
async function emptyDir(dir) {
  let entries;
  try {
    entries = await fsp.readdir(dir);
  } catch {
    return 0;
  }
  let removed = 0;
  for (const name of entries) {
    try {
      await fsp.rm(path.join(dir, name), { recursive: true, force: true });
      removed++;
    } catch { /* in use by a running game; leave it */ }
  }
  return removed;
}

/**
 * Clear one removable section. The game re-downloads what it needs on the
 * next launch, so this costs bandwidth, never worlds.
 */
async function clear(key) {
  const section = SECTIONS.find((s) => s.key === key);
  if (!section) throw new Error(`Unknown storage section: ${key}`);
  if (!section.removable) throw new Error('This folder holds your worlds and is never cleared automatically');
  await emptyDir(section.dir);
  await fsp.mkdir(section.dir, { recursive: true });
  return { key, bytes: await dirSize(section.dir) };
}

module.exports = { usage, clear, dirSize, SECTIONS };
