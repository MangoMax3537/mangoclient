// X11 helpers through xdotool (the harness runs the app under XWayland, GDK_BACKEND=x11). Everything is addressed by PID.
import { spawnSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { descendants, statOf } from './proc.mjs';

const run = (args) => spawnSync('xdotool', args, { encoding: 'utf8' });

export function hasXdotool() {
  return !spawnSync('xdotool', ['version'], { encoding: 'utf8' }).error;
}

/** Window ids of `pid` named "MangoClient". `visible` = only mapped ones. */
export function mainWindows(pid, visible = true) {
  const r = run(['search', ...(visible ? ['--onlyvisible'] : []), '--pid', String(pid)]);
  return (r.stdout || '').split('\n').filter(Boolean).filter((id) => (run(['getwindowname', id]).stdout || '').trim() === 'MangoClient');
}

export function geometry(id) {
  const r = run(['getwindowgeometry', '--shell', id]);
  const o = {};
  for (const l of (r.stdout || '').split('\n')) { const [k, v] = l.split('='); if (k && v !== undefined) o[k] = Number(v); }
  return { x: o.X, y: o.Y, w: o.WIDTH, h: o.HEIGHT };
}

export const quitWindow = (id) => run(['windowquit', id]);
export const mapWindow = (id) => run(['windowmap', id]);
export const activate = (id) => run(['windowactivate', id]);
export const focus = (id) => run(['windowfocus', id]);
export const key = (k) => run(['key', k]);
export const mouseUp = () => run(['mouseup', '1']);
export const doubleClick = (x, y) => run(['mousemove', String(Math.round(x)), String(Math.round(y)), 'click', '--repeat', '2', '--delay', '90', '1']);

/** The WebKit helper processes (web content, network) below the app. */
export function webkitChildren(appPid) {
  return descendants(appPid).filter((p) => /WebKit/i.test(statOf(p)?.comm || ''));
}

/** Java processes below the app (the game). */
export function javaChildren(appPid) {
  return descendants(appPid).filter((p) => /^java/i.test(statOf(p)?.comm || ''));
}

/** RSS and PSS in MiB of one process (PSS needs readable smaps_rollup; own children are). */
export function memOf(pid) {
  const mib = (kb) => Math.round((kb / 1024) * 10) / 10;
  const out = { rss: null, pss: null };
  try {
    const m = readFileSync(`/proc/${pid}/status`, 'utf8').match(/VmRSS:\s+(\d+) kB/);
    if (m) out.rss = mib(Number(m[1]));
  } catch { /* gone */ }
  try {
    const m = readFileSync(`/proc/${pid}/smaps_rollup`, 'utf8').match(/^Pss:\s+(\d+) kB/m);
    if (m) out.pss = mib(Number(m[1]));
  } catch { /* not readable */ }
  return out;
}
