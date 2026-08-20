'use strict';
const fs = require('fs');
const path = require('path');
const P = require('./paths');

/**
 * A journal of play sessions, so the launcher can show the kind of statistics
 * OneLauncher does: how much was played, when, and on which instance.
 *
 * The profile's own `playTimeMs` stays the authority on lifetime totals - it
 * predates this file and counts sessions from before it existed. The journal
 * only adds the one thing a running total cannot: *when* the time was spent.
 */

/** Roughly four years of daily play; old entries fall off the front. */
const MAX_SESSIONS = 4000;
/** Anything shorter is a crash or a mistyped version, not a play session. */
const MIN_SESSION_MS = 30 * 1000;

const FILE = path.join(P.root, 'stats.json');

function read() {
  try {
    const data = JSON.parse(fs.readFileSync(FILE, 'utf8'));
    return Array.isArray(data?.sessions) ? data : { version: 1, sessions: [] };
  } catch {
    return { version: 1, sessions: [] };
  }
}

function write(data) {
  const tmp = `${FILE}.tmp`;
  fs.mkdirSync(path.dirname(FILE), { recursive: true });
  fs.writeFileSync(tmp, JSON.stringify(data));
  fs.renameSync(tmp, FILE);
}

/**
 * Record a finished stretch of play. Called whenever the launcher flushes
 * play time, which happens both on exit and while a long session runs, so
 * `start` is the beginning of the stretch, not of the whole session.
 */
function record(profileId, ms, start = Date.now() - ms) {
  if (!profileId || !(ms >= MIN_SESSION_MS)) return;
  const data = read();
  data.sessions.push({ p: profileId, s: Math.round(start), ms: Math.round(ms) });
  if (data.sessions.length > MAX_SESSIONS) data.sessions.splice(0, data.sessions.length - MAX_SESSIONS);
  try {
    write(data);
  } catch (err) {
    console.error('[stats]', err); // statistics are never worth failing a launch over
  }
}

function dayKey(ts) {
  const d = new Date(ts);
  const pad = (n) => String(n).padStart(2, '0');
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
}

/** Local midnight `days-1` days ago, so "last 14 days" includes today. */
function windowStart(days) {
  const d = new Date();
  d.setHours(0, 0, 0, 0);
  d.setDate(d.getDate() - (days - 1));
  return d.getTime();
}

/**
 * Everything the statistics view draws, computed in one pass so the renderer
 * only formats. `profiles` comes from the store; deleted instances keep their
 * sessions in the journal but are left out of the breakdown.
 */
function summary(profiles = [], days = 14) {
  const { sessions } = read();
  const byId = new Map(profiles.map((p) => [p.id, p]));

  const from = windowStart(days);
  const buckets = new Map();
  for (let i = 0; i < days; i++) {
    const d = new Date(from);
    d.setDate(d.getDate() + i);
    buckets.set(dayKey(d.getTime()), 0);
  }

  let windowMs = 0;
  let longest = null;
  const perProfileWindow = new Map();

  for (const s of sessions) {
    if (longest === null || s.ms > longest.ms) {
      longest = { ms: s.ms, start: s.s, profileId: s.p, name: byId.get(s.p)?.name || null };
    }
    if (s.s < from) continue;
    const key = dayKey(s.s);
    if (buckets.has(key)) buckets.set(key, buckets.get(key) + s.ms);
    windowMs += s.ms;
    perProfileWindow.set(s.p, (perProfileWindow.get(s.p) || 0) + s.ms);
  }

  const perProfile = profiles
    .map((p) => ({
      id: p.id,
      name: p.name,
      color: p.color || null,
      mcVersion: p.mcVersion,
      loader: p.loader,
      totalMs: p.playTimeMs || 0,
      windowMs: perProfileWindow.get(p.id) || 0,
      lastPlayed: p.lastPlayed || null,
    }))
    .sort((a, b) => b.totalMs - a.totalMs);

  const totalMs = perProfile.reduce((sum, p) => sum + p.totalMs, 0);

  return {
    days: [...buckets].map(([date, ms]) => ({ date, ms })),
    windowMs,
    totalMs,
    sessions: sessions.length,
    longest,
    perProfile,
    mostPlayed: perProfile[0] || null,
    // Nothing was journalled before this feature existed; say so rather than
    // showing an empty chart next to a large lifetime total.
    journalled: sessions.length > 0,
  };
}

module.exports = { record, summary, MIN_SESSION_MS };
