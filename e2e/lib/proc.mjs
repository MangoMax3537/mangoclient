// /proc helpers (Linux only). Everything works by PID; nothing here matches command lines by pattern.
import { readdirSync, readFileSync, readlinkSync } from 'node:fs';
import { basename } from 'node:path';

export function pids() {
  return readdirSync('/proc').filter((d) => /^\d+$/.test(d)).map(Number);
}

export function alive(pid) {
  try {
    process.kill(pid, 0);
    // A zombie still answers kill(0); treat it as gone.
    return statOf(pid)?.state !== 'Z';
  } catch {
    return false;
  }
}

/** { ppid, comm, state } or null. */
export function statOf(pid) {
  try {
    const s = readFileSync(`/proc/${pid}/stat`, 'utf8');
    const open = s.indexOf('(');
    const close = s.lastIndexOf(')');
    const rest = s.slice(close + 2).split(' ');
    return { comm: s.slice(open + 1, close), state: rest[0], ppid: Number(rest[1]) };
  } catch {
    return null;
  }
}

export function exeOf(pid) {
  try {
    return readlinkSync(`/proc/${pid}/exe`).replace(/ \(deleted\)$/, '');
  } catch {
    return null;
  }
}

export function envOf(pid) {
  try {
    const out = {};
    for (const kv of readFileSync(`/proc/${pid}/environ`, 'utf8').split('\0')) {
      const i = kv.indexOf('=');
      if (i > 0) out[kv.slice(0, i)] = kv.slice(i + 1);
    }
    return out;
  } catch {
    return {};
  }
}

/** All transitive children of `root` (not including it). */
export function descendants(root) {
  const kids = new Map();
  for (const p of pids()) {
    const s = statOf(p);
    if (!s) continue;
    if (!kids.has(s.ppid)) kids.set(s.ppid, []);
    kids.get(s.ppid).push(p);
  }
  const out = [];
  const walk = (p) => {
    for (const k of kids.get(p) || []) {
      out.push(k);
      walk(k);
    }
  };
  walk(root);
  return out;
}

/** PIDs whose executable is a file called `name` (any directory). */
export function pidsByExeName(name) {
  return pids().filter((p) => {
    const e = exeOf(p);
    return e && basename(e) === name;
  });
}

/** Wait until `pid` is gone; true if it exited within `ms`. */
export async function waitExit(pid, ms) {
  const end = Date.now() + ms;
  while (Date.now() < end) {
    if (!alive(pid)) return true;
    await new Promise((r) => setTimeout(r, 100));
  }
  return !alive(pid);
}

export function kill(pid, sig = 'SIGTERM') {
  try {
    process.kill(pid, sig);
  } catch {
    /* gone */
  }
}
