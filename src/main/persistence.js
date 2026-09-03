'use strict';
const fs = require('fs');
const path = require('path');

function validJSON(raw) {
  if (raw == null) return false;
  try {
    JSON.parse(raw);
    return true;
  } catch {
    return false;
  }
}

/** Write a complete file, flush it, then atomically put it in place. */
function writeFileAtomic(file, data, mode = 0o600) {
  fs.mkdirSync(path.dirname(file), { recursive: true });
  const tmp = `${file}.tmp-${process.pid}-${Date.now()}-${Math.random().toString(16).slice(2)}`;
  let fd;
  try {
    fd = fs.openSync(tmp, 'wx', mode);
    fs.writeFileSync(fd, data);
    fs.fsyncSync(fd);
    fs.closeSync(fd);
    fd = undefined;
    fs.renameSync(tmp, file);
    fs.chmodSync(file, mode);
  } catch (err) {
    if (fd !== undefined) {
      try { fs.closeSync(fd); } catch { /* already closed */ }
    }
    try { fs.unlinkSync(tmp); } catch { /* no temporary file left */ }
    throw err;
  }
}

/**
 * Read JSON with a last-known-good sidecar. A truncated main file can otherwise
 * make every profile look new even though all instance folders still exist.
 */
function readJSON(file, fallback) {
  const backup = `${file}.bak`;
  let mainRaw = null;
  try { mainRaw = fs.readFileSync(file, 'utf8'); } catch { /* try the backup */ }
  if (validJSON(mainRaw)) return JSON.parse(mainRaw);

  let backupRaw = null;
  try { backupRaw = fs.readFileSync(backup, 'utf8'); } catch { /* no recovery copy */ }
  if (!validJSON(backupRaw)) return fallback;

  // Restore only the main file. Rotating here would overwrite the good backup
  // with the corrupt/truncated file that brought us down this path.
  try { writeFileAtomic(file, backupRaw); } catch { /* recovery still works in memory */ }
  return JSON.parse(backupRaw);
}

/**
 * Keep the previous valid generation as `.bak`. On the first save, establish
 * the backup before the main file, so either side is sufficient after a crash.
 */
function writeJSONAtomic(file, data) {
  const payload = `${JSON.stringify(data, null, 2)}\n`;
  const backup = `${file}.bak`;
  let previous = null;
  try {
    const raw = fs.readFileSync(file, 'utf8');
    if (validJSON(raw)) previous = raw;
  } catch { /* first save */ }

  if (previous != null) {
    writeFileAtomic(backup, previous);
  } else {
    let hasGoodBackup = false;
    try { hasGoodBackup = validJSON(fs.readFileSync(backup, 'utf8')); } catch { /* none */ }
    if (!hasGoodBackup) writeFileAtomic(backup, payload);
  }
  writeFileAtomic(file, payload);
}

module.exports = { readJSON, writeJSONAtomic, writeFileAtomic, validJSON };
