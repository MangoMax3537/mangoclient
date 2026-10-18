'use strict';
/*
 * The admin dashboard.
 *
 * One page, seven views, all fed by the /admin/* API behind the password.
 * Everything is drawn with textContent and plain SVG: no library, nothing a
 * player name or server address could inject into, and nothing the CSP has
 * to make an exception for besides the player heads.
 */

const RANK_COLORS = { owner: 'var(--owner)', support: 'var(--support)', mangoplus: 'var(--mangoplus)' };
const RANK_LABELS = { owner: 'Owner', support: 'Support', mangoplus: 'Mango+' };
const OS_LABELS = { win32: 'Windows', darwin: 'macOS', linux: 'Linux' };
const LIVE_MS = 10_000;
const SLOW_MS = 30_000;

const $ = (id) => document.getElementById(id);
const gate = $('gate');
const panel = $('panel');

const nf = new Intl.NumberFormat('de-DE');
const dateFmt = new Intl.DateTimeFormat('de-DE', { day: '2-digit', month: '2-digit', year: 'numeric' });
const dateTimeFmt = new Intl.DateTimeFormat('de-DE', { day: '2-digit', month: '2-digit', hour: '2-digit', minute: '2-digit' });
const timeFmt = new Intl.DateTimeFormat('de-DE', { hour: '2-digit', minute: '2-digit' });

let view = 'overview';
let timers = [];
let liveRows = [];
let liveSort = { key: 'since', dir: 1 };
let dirRows = [];
let installRows = [];

// --- small helpers -------------------------------------------------------------

function el(tag, className, text) {
  const node = document.createElement(tag);
  if (className) node.className = className;
  if (text !== undefined && text !== null) node.textContent = text;
  return node;
}

function say(node, message, kind) {
  node.textContent = message || '';
  node.className = 'note' + (kind ? ' ' + kind : '');
}

let toastTimer;
function toast(message, bad) {
  const node = $('toast');
  node.textContent = message;
  node.className = 'toast' + (bad ? ' bad' : '');
  node.hidden = false;
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => { node.hidden = true; }, 3200);
}

async function api(path, body) {
  const options = body
    ? { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(body) }
    : { cache: 'no-store' };
  const response = await fetch(path, options);
  const data = await response.json().catch(() => ({}));
  if (response.status === 401 && path !== '/admin/login') {
    show(false);
    throw new Error('Sitzung abgelaufen, bitte neu anmelden.');
  }
  if (!response.ok) throw new Error(data.error || `Fehler (${response.status})`);
  return data;
}

function ago(iso) {
  const s = Math.max(0, (Date.now() - new Date(iso).getTime()) / 1000);
  if (s < 45) return 'gerade eben';
  if (s < 3600) return `vor ${Math.round(s / 60)} Min.`;
  if (s < 86400) return `vor ${Math.round(s / 3600)} Std.`;
  if (s < 86400 * 30) return `vor ${Math.round(s / 86400)} Tagen`;
  return dateFmt.format(new Date(iso));
}

function duration(seconds) {
  seconds = Math.max(0, Math.round(seconds));
  const d = Math.floor(seconds / 86400);
  const h = Math.floor((seconds % 86400) / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  if (d > 0) return `${d} T ${h} Std.`;
  if (h > 0) return `${h} Std. ${m} Min.`;
  return `${m} Min.`;
}

function since(iso) {
  return duration((Date.now() - new Date(iso).getTime()) / 1000);
}

function dashed(uuid) {
  return uuid.length === 32
    ? `${uuid.slice(0, 8)}-${uuid.slice(8, 12)}-${uuid.slice(12, 16)}-${uuid.slice(16, 20)}-${uuid.slice(20)}`
    : uuid;
}

function head(uuid) {
  const img = el('img');
  img.alt = '';
  img.loading = 'lazy';
  img.src = `https://mc-heads.net/avatar/${uuid}/48`;
  img.onerror = () => { img.style.visibility = 'hidden'; };
  return img;
}

function whoCell(player) {
  const box = el('div', 'who');
  box.appendChild(head(player.uuid));
  const text = el('div');
  text.appendChild(el('span', 'n', player.name || 'Unbekannt'));
  if (player.rank) {
    text.appendChild(document.createTextNode(' '));
    text.appendChild(rankTag(player.rank));
  }
  const u = el('span', 'u', dashed(player.uuid));
  u.title = 'UUID kopieren';
  u.addEventListener('click', () => copy(dashed(player.uuid)));
  text.appendChild(u);
  box.appendChild(text);
  return box;
}

function rankTag(rank) {
  const tag = el('span', 'tag rank', RANK_LABELS[rank] || rank);
  tag.style.setProperty('--rank-color', RANK_COLORS[rank] || 'var(--member)');
  return tag;
}

function versionTag(value) {
  return value ? el('span', 'tag', value) : el('span', 'tag old', 'alt');
}

function serverCell(server, legacy) {
  if (server === 'singleplayer') return el('span', 'tag sp', 'Einzelspieler');
  if (!server) return el('span', 'srv none', legacy ? 'unbekannt' : 'Menü / lädt');
  return el('span', 'srv', server);
}

function td(content, className) {
  const cell = el('td', className);
  if (content instanceof Node) cell.appendChild(content);
  else cell.textContent = content ?? '';
  return cell;
}

async function copy(text) {
  try {
    await navigator.clipboard.writeText(text);
    toast('Kopiert: ' + text);
  } catch {
    toast('Kopieren nicht möglich', true);
  }
}

function csv(filename, header, rows) {
  const quote = (v) => {
    const s = String(v ?? '');
    return /[",;\n]/.test(s) ? `"${s.replace(/"/g, '""')}"` : s;
  };
  const text = [header, ...rows].map((r) => r.map(quote).join(';')).join('\n');
  const blob = new Blob(['﻿' + text], { type: 'text/csv;charset=utf-8' });
  const a = el('a');
  a.href = URL.createObjectURL(blob);
  a.download = filename;
  document.body.appendChild(a);
  a.click();
  a.remove();
  setTimeout(() => URL.revokeObjectURL(a.href), 1000);
}

function debounce(fn, ms) {
  let t;
  return (...args) => { clearTimeout(t); t = setTimeout(() => fn(...args), ms); };
}

// --- the gate ------------------------------------------------------------------

async function boot() {
  const session = await api('/admin/session').catch(() => ({ authed: false }));
  if (session.authed) {
    show(true);
  } else {
    show(false);
    if (session.configured === false) {
      say($('login-note'), 'Auf dem Server ist kein Passwort gesetzt. MANGO_ADMIN_PASSWORD setzen und den Dienst neu starten.', 'bad');
    }
  }
}

function show(authed) {
  gate.hidden = authed;
  panel.hidden = !authed;
  stopTimers();
  if (authed) {
    go(location.hash.slice(1) || 'overview');
  } else {
    $('password').focus();
  }
}

$('login').addEventListener('submit', async (event) => {
  event.preventDefault();
  const field = $('password');
  const noteEl = $('login-note');
  say(noteEl, 'Prüfe…');
  try {
    await api('/admin/login', { password: field.value });
    field.value = '';
    say(noteEl, '');
    show(true);
  } catch (error) {
    say(noteEl, error.message === 'wrong password' ? 'Falsches Passwort.' : error.message, 'bad');
    field.select();
  }
});

$('logout').addEventListener('click', async () => {
  await api('/admin/logout', {}).catch(() => {});
  show(false);
});

// --- navigation and polling ------------------------------------------------------

const LOADERS = {
  overview: [loadOverview, SLOW_MS],
  live: [loadLive, LIVE_MS],
  servers: [loadServers, LIVE_MS],
  players: [loadDirectory, 0],
  installs: [loadInstalls, SLOW_MS],
  ranks: [loadRanks, 0],
  system: [loadSystem, SLOW_MS],
};

function stopTimers() {
  timers.forEach(clearInterval);
  timers = [];
}

function go(name) {
  if (!LOADERS[name]) name = 'overview';
  view = name;
  if (location.hash.slice(1) !== name) history.replaceState(null, '', '#' + name);
  for (const button of document.querySelectorAll('#nav button')) {
    button.classList.toggle('on', button.dataset.view === name);
  }
  for (const section of document.querySelectorAll('.view')) {
    section.hidden = section.id !== 'view-' + name;
  }
  say($('note'), '');
  stopTimers();
  const [load, every] = LOADERS[name];
  run(load);
  if (every) timers.push(setInterval(() => { if (!document.hidden) run(load); }, every));
  // The live count in the sidebar stays fresh whatever view is open.
  if (name !== 'live') timers.push(setInterval(() => { if (!document.hidden) refreshLiveCount(); }, LIVE_MS));
  refreshLiveCount();
}

async function run(load) {
  try {
    await load();
    $('pulse').classList.remove('stale');
  } catch (error) {
    $('pulse').classList.add('stale');
    if (!panel.hidden) say($('note'), error.message, 'bad');
  }
}

async function refreshLiveCount() {
  try {
    const data = await api('/admin/live');
    liveRows = data.players;
    $('nav-live').textContent = nf.format(liveRows.length);
  } catch {
    // run() reports errors for the open view; the badge just keeps its number.
  }
}

$('nav').addEventListener('click', (event) => {
  const button = event.target.closest('button[data-view]');
  if (button) go(button.dataset.view);
});

window.addEventListener('hashchange', () => {
  if (!panel.hidden && location.hash.slice(1) !== view) go(location.hash.slice(1));
});

// --- overview ------------------------------------------------------------------

async function loadOverview() {
  const ov = await api('/admin/overview');
  const tiles = [
    ['Jetzt im Spiel', ov.inGame, 'live', 'mit MangoConfig'],
    ['Launcher offen', ov.launchersOpen, 'live', 'gerade gestartet'],
    ['Installationen', ov.installs, 'hot', 'insgesamt'],
    ['Aktiv (30 Tage)', ov.activeMonth, '', 'Launcher geöffnet'],
    ['Spieler gesamt', ov.playersEver, '', 'je gesehen im Spiel'],
    ['Neue Spieler', ov.newPlayersWeek, '', 'letzte 7 Tage'],
    ['Launcher-Sessions', ov.sessions, '', 'insgesamt'],
    ['Spielzeit gesamt', null, '', 'seit Messbeginn', duration(ov.playtime)],
  ];
  const host = $('tiles');
  host.replaceChildren(...tiles.map(([label, value, kind, foot, text]) => {
    const tile = el('div', 'tile' + (kind ? ' ' + kind : ''));
    tile.append(el('span', 'label', label), el('span', 'value', text ?? nf.format(value || 0)), el('span', 'foot', foot));
    return tile;
  }));
  $('ov-updated').textContent = 'Stand ' + timeFmt.format(new Date());

  lineChart($('chart-history'), ov.history || []);
  dayChart($('chart-days'), ov.days || []);
  breakdown($('bd-mc'), ov.byMC, (k) => (k === 'unknown' ? 'unbekannt (alte Mod)' : k));
  breakdown($('bd-mod'), ov.byMod, (k) => (k === 'unknown' ? 'älter als 1.10.1' : k));
  breakdown($('bd-server'), ov.byServer, (k) => (k === 'unknown' ? 'unbekannt / Menü' : k === 'singleplayer' ? 'Einzelspieler' : k));
  breakdown($('bd-launcher'), ov.byVersion);
  breakdown($('bd-os'), ov.byOS, (k) => OS_LABELS[k] || k);
}

function breakdown(host, counts, label = (k) => k) {
  const entries = Object.entries(counts || {}).sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0]));
  if (entries.length === 0) {
    host.replaceChildren(el('p', 'none', 'Keine Daten.'));
    return;
  }
  const total = entries.reduce((sum, [, n]) => sum + n, 0);
  const max = entries[0][1];
  host.replaceChildren(...entries.slice(0, 10).map(([key, n]) => {
    const row = el('div', 'barrow');
    const k = el('span', 'k', label(key));
    k.title = label(key);
    const v = el('span', 'v', `${nf.format(n)} · ${Math.round((n / total) * 100)} %`);
    const track = el('div', 'track');
    const fill = el('div', 'fill');
    fill.style.width = `${(n / max) * 100}%`;
    track.appendChild(fill);
    row.append(k, v, track);
    return row;
  }));
}

// --- charts (hand-drawn SVG) ----------------------------------------------------

const SVG = 'http://www.w3.org/2000/svg';
function svgEl(tag, attrs) {
  const node = document.createElementNS(SVG, tag);
  for (const [k, v] of Object.entries(attrs || {})) node.setAttribute(k, v);
  return node;
}

// Four gridlines, so the top is four whole, round ticks.
function niceMax(n) {
  let tick = Math.max(1, Math.ceil(n / 4));
  if (tick > 5) tick = Math.ceil(tick / 5) * 5;
  if (tick > 50) tick = Math.ceil(tick / 25) * 25;
  return tick * 4;
}

function frame(host) {
  const w = host.clientWidth || 600;
  const h = host.clientHeight || 200;
  const pad = { l: 34, r: 8, t: 8, b: 22 };
  const svg = svgEl('svg', { viewBox: `0 0 ${w} ${h}`, preserveAspectRatio: 'none' });
  host.replaceChildren(svg);
  return { svg, w, h, pad, iw: w - pad.l - pad.r, ih: h - pad.t - pad.b };
}

function yAxis(f, max) {
  const grid = svgEl('g', { class: 'grid' });
  const axis = svgEl('g', { class: 'axis' });
  for (let i = 0; i <= 4; i++) {
    const y = f.pad.t + f.ih - (i / 4) * f.ih;
    grid.appendChild(svgEl('line', { x1: f.pad.l, x2: f.w - f.pad.r, y1: y, y2: y }));
    const label = svgEl('text', { x: f.pad.l - 6, y: y + 3, 'text-anchor': 'end' });
    label.textContent = nf.format(Math.round((max * i) / 4));
    axis.appendChild(label);
  }
  f.svg.append(grid, axis);
  return axis;
}

function tooltip(host) {
  let tip = host.querySelector('.tip');
  if (!tip) {
    tip = el('div', 'tip');
    tip.hidden = true;
    host.appendChild(tip);
  }
  return tip;
}

function noData(f, text) {
  const t = svgEl('text', { x: f.w / 2, y: f.h / 2, 'text-anchor': 'middle', class: 'nodata' });
  t.textContent = text;
  f.svg.appendChild(t);
}

function lineChart(host, samples) {
  const f = frame(host);
  if (samples.length < 2) {
    yAxis(f, 4);
    noData(f, 'Der Verlauf füllt sich alle 5 Minuten.');
    return;
  }
  const t0 = new Date(samples[0].t).getTime();
  const t1 = new Date(samples[samples.length - 1].t).getTime();
  const max = niceMax(Math.max(...samples.map((s) => Math.max(s.inGame, s.launchers))));
  const x = (t) => f.pad.l + ((t - t0) / Math.max(1, t1 - t0)) * f.iw;
  const y = (v) => f.pad.t + f.ih - (v / max) * f.ih;
  const axis = yAxis(f, max);

  // Day ticks along the bottom.
  const day = new Date(t0);
  day.setHours(24, 0, 0, 0);
  for (; day.getTime() <= t1; day.setDate(day.getDate() + 1)) {
    const label = svgEl('text', { x: x(day.getTime()), y: f.h - 6, 'text-anchor': 'middle' });
    label.textContent = day.toLocaleDateString('de-DE', { weekday: 'short', day: '2-digit' });
    axis.appendChild(label);
  }

  const pts = samples.map((s) => [x(new Date(s.t).getTime()), s]);
  const line = (key) => pts.map(([px, s], i) => `${i ? 'L' : 'M'}${px.toFixed(1)},${y(s[key]).toFixed(1)}`).join('');
  const area = `${line('inGame')}L${pts[pts.length - 1][0].toFixed(1)},${y(0)}L${pts[0][0].toFixed(1)},${y(0)}Z`;
  f.svg.append(
    svgEl('path', { d: area, class: 'a1' }),
    svgEl('path', { d: line('launchers'), class: 'l2' }),
    svgEl('path', { d: line('inGame'), class: 'l1' }),
  );

  const hover = svgEl('line', { class: 'hover-line', y1: f.pad.t, y2: f.pad.t + f.ih, visibility: 'hidden' });
  f.svg.appendChild(hover);
  const tip = tooltip(host);
  host.onmousemove = (event) => {
    const rect = host.getBoundingClientRect();
    const mx = ((event.clientX - rect.left) / rect.width) * f.w;
    let best = pts[0];
    for (const p of pts) if (Math.abs(p[0] - mx) < Math.abs(best[0] - mx)) best = p;
    hover.setAttribute('x1', best[0]);
    hover.setAttribute('x2', best[0]);
    hover.setAttribute('visibility', 'visible');
    tip.replaceChildren(
      document.createTextNode(dateTimeFmt.format(new Date(best[1].t)) + ' · '),
      el('b', null, `${best[1].inGame} im Spiel`),
      document.createTextNode(` · ${best[1].launchers} Launcher`),
    );
    tip.style.left = `${(best[0] / f.w) * rect.width}px`;
    tip.style.top = `${(y(best[1].inGame) / f.h) * rect.height}px`;
    tip.hidden = false;
  };
  host.onmouseleave = () => { tip.hidden = true; hover.setAttribute('visibility', 'hidden'); };
}

function dayChart(host, days) {
  const f = frame(host);
  const max = niceMax(Math.max(1, ...days.map((d) => Math.max(d.players, d.launchers, d.new))));
  const axis = yAxis(f, max);
  const slot = f.iw / Math.max(1, days.length);
  const bw = Math.max(2, slot / 4);
  const y = (v) => f.pad.t + f.ih - (v / max) * f.ih;
  const tip = tooltip(host);

  days.forEach((d, i) => {
    const x0 = f.pad.l + i * slot + slot / 2 - bw * 1.5;
    const g = svgEl('g');
    [['players', 'b1'], ['launchers', 'b2'], ['new', 'b3']].forEach(([key, cls], j) => {
      const h = Math.max(d[key] ? 1 : 0, f.pad.t + f.ih - y(d[key]));
      g.appendChild(svgEl('rect', { x: x0 + j * bw, y: f.pad.t + f.ih - h, width: bw - 1, height: h, rx: 1, class: cls }));
    });
    // A full-height invisible hit area per day makes the tooltip easy to reach.
    const hit = svgEl('rect', { x: f.pad.l + i * slot, y: f.pad.t, width: slot, height: f.ih, fill: 'transparent' });
    hit.addEventListener('mouseenter', () => {
      const rect = host.getBoundingClientRect();
      const date = new Date(d.day + 'T12:00:00Z');
      tip.replaceChildren(
        document.createTextNode(date.toLocaleDateString('de-DE', { weekday: 'short', day: '2-digit', month: '2-digit' }) + ' · '),
        el('b', null, `${d.players} Spieler`),
        document.createTextNode(` · ${d.launchers} Launcher · ${d.new} neu · Peak ${d.peak}`),
      );
      tip.style.left = `${((f.pad.l + i * slot + slot / 2) / f.w) * rect.width}px`;
      tip.style.top = `${(y(Math.max(d.players, d.launchers)) / f.h) * rect.height}px`;
      tip.hidden = false;
    });
    g.appendChild(hit);
    f.svg.appendChild(g);
    if (i % 5 === 4 || i === days.length - 1) {
      const label = svgEl('text', { x: f.pad.l + i * slot + slot / 2, y: f.h - 6, 'text-anchor': 'middle' });
      label.textContent = d.day.slice(8, 10) + '.' + d.day.slice(5, 7) + '.';
      axis.appendChild(label);
    }
  });
  host.onmouseleave = () => { tip.hidden = true; };
}

window.addEventListener('resize', debounce(() => { if (view === 'overview' && !panel.hidden) run(loadOverview); }, 250));

// --- live ----------------------------------------------------------------------

async function loadLive() {
  const data = await api('/admin/live');
  liveRows = data.players;
  $('nav-live').textContent = nf.format(liveRows.length);
  fillSelect($('live-server'), liveRows.map((p) => p.server || ''), 'Alle Server', (s) => (s === '' ? 'unbekannt / Menü' : s === 'singleplayer' ? 'Einzelspieler' : s));
  fillSelect($('live-mc'), liveRows.map((p) => p.mc || ''), 'Alle Versionen', (s) => (s === '' ? 'unbekannt' : s));
  renderLive();
}

function fillSelect(select, values, allLabel, label) {
  const current = select.value;
  const counts = new Map();
  for (const v of values) counts.set(v, (counts.get(v) || 0) + 1);
  const options = [new Option(allLabel, '__all')];
  for (const [v, n] of [...counts].sort((a, b) => b[1] - a[1])) options.push(new Option(`${label(v)} (${n})`, v));
  select.replaceChildren(...options);
  select.value = [...counts.keys()].includes(current) ? current : '__all';
}

function filteredLive() {
  const q = $('live-q').value.trim().toLowerCase().replace(/-/g, '');
  const server = $('live-server').value;
  const mc = $('live-mc').value;
  const rows = liveRows.filter((p) =>
    (server === '__all' || (p.server || '') === server)
    && (mc === '__all' || (p.mc || '') === mc)
    && (!q || (p.name || '').toLowerCase().includes(q) || p.uuid.includes(q) || (p.server || '').includes(q)));
  const { key, dir } = liveSort;
  return rows.sort((a, b) => {
    const av = a[key] || '';
    const bv = b[key] || '';
    if (key === 'since' || key === 'last') return (new Date(av) - new Date(bv)) * dir;
    return String(av).localeCompare(String(bv), 'de', { numeric: true }) * dir;
  });
}

function renderLive() {
  const rows = filteredLive();
  const tbody = $('live-table').querySelector('tbody');
  tbody.replaceChildren(...rows.map((p) => {
    const tr = el('tr');
    const legacy = !p.mod;
    tr.append(
      td(whoCell(p)),
      td(versionTag(p.mc)),
      td(versionTag(p.mod)),
      td(serverCell(p.server, legacy)),
      td(since(p.since)),
      td(ago(p.last), 'muted'),
    );
    return tr;
  }));
  $('live-empty').hidden = liveRows.length > 0;
  $('live-count').textContent = rows.length === liveRows.length
    ? `${nf.format(liveRows.length)} online`
    : `${nf.format(rows.length)} von ${nf.format(liveRows.length)} online`;
  for (const th of $('live-table').querySelectorAll('th[data-sort]')) {
    th.classList.toggle('asc', th.dataset.sort === liveSort.key && liveSort.dir === 1);
    th.classList.toggle('desc', th.dataset.sort === liveSort.key && liveSort.dir === -1);
  }
}

$('live-table').querySelector('thead').addEventListener('click', (event) => {
  const th = event.target.closest('th[data-sort]');
  if (!th) return;
  liveSort = { key: th.dataset.sort, dir: liveSort.key === th.dataset.sort ? -liveSort.dir : 1 };
  renderLive();
});
$('live-q').addEventListener('input', renderLive);
$('live-server').addEventListener('change', renderLive);
$('live-mc').addEventListener('change', renderLive);
$('live-csv').addEventListener('click', () => {
  csv(`mangoclient-online-${new Date().toISOString().slice(0, 10)}.csv`,
    ['Name', 'UUID', 'Rang', 'Minecraft', 'MangoConfig', 'Server', 'Online seit', 'Letztes Signal'],
    filteredLive().map((p) => [p.name, dashed(p.uuid), p.rank, p.mc, p.mod, p.server, p.since, p.last]));
});

// --- servers -------------------------------------------------------------------

async function loadServers() {
  const [live, ov] = await Promise.all([api('/admin/live'), api('/admin/overview')]);
  liveRows = live.players;
  $('nav-live').textContent = nf.format(liveRows.length);

  const groups = new Map();
  for (const p of liveRows) {
    const key = p.server || '';
    if (!groups.has(key)) groups.set(key, []);
    groups.get(key).push(p);
  }
  const sorted = [...groups].sort((a, b) => b[1].length - a[1].length || a[0].localeCompare(b[0]));
  const grid = $('server-grid');
  if (sorted.length === 0) {
    grid.replaceChildren(el('p', 'empty', 'Gerade ist niemand online.'));
  } else {
    grid.replaceChildren(...sorted.map(([server, players]) => {
      const card = el('article', 'server-card');
      const header = el('header');
      const title = el('h3', null, server === '' ? 'Unbekannt / Menü' : server === 'singleplayer' ? 'Einzelspieler' : server);
      title.title = title.textContent;
      header.append(title, el('span', 'tag on', `${players.length} online`));
      if (server && server !== 'singleplayer') {
        const button = el('button', 'ghost tiny', 'IP kopieren');
        button.type = 'button';
        button.addEventListener('click', () => copy(server));
        header.appendChild(button);
      }
      const ul = el('ul');
      for (const p of players.sort((a, b) => (a.name || '').localeCompare(b.name || ''))) {
        const li = el('li');
        const who = el('div', 'who');
        who.append(head(p.uuid), el('span', 'n', p.name || 'Unbekannt'));
        li.append(who, el('span', 'tag', p.mc || 'alt'));
        ul.appendChild(li);
      }
      card.append(header, ul);
      return card;
    }));
  }

  const tbody = $('top-servers').querySelector('tbody');
  const top = ov.topServers || [];
  tbody.replaceChildren(...(top.length ? top.map((s) => {
    const tr = el('tr');
    tr.append(td(serverCell(s.server)), td(nf.format(s.online), 'num'), td(nf.format(s.players), 'num'));
    return tr;
  }) : [(() => { const tr = el('tr'); const c = td('Noch keine Serverdaten (erst ab MangoConfig 1.10.1).', 'muted'); c.colSpan = 3; tr.appendChild(c); return tr; })()]));
}

// --- all players -----------------------------------------------------------------

async function loadDirectory() {
  const q = $('dir-q').value.trim();
  const data = await api('/admin/players?limit=1000&q=' + encodeURIComponent(q));
  dirRows = data.players;
  $('dir-count').textContent = data.total > dirRows.length
    ? `${nf.format(dirRows.length)} von ${nf.format(data.total)} angezeigt`
    : `${nf.format(data.total)} Spieler`;
  const tbody = $('dir-table').querySelector('tbody');
  tbody.replaceChildren(...dirRows.map((p) => {
    const tr = el('tr');
    tr.append(
      td(whoCell(p)),
      td(p.online ? el('span', 'tag on', 'online') : el('span', 'tag off', 'offline')),
      td(serverCell(p.server, !p.mod)),
      td(p.mc || '–', p.mc ? '' : 'muted'),
      td(p.mod || '–', p.mod ? '' : 'muted'),
      td(p.first ? dateFmt.format(new Date(p.first)) : '–'),
      td(ago(p.last)),
      td(nf.format(p.sessions || 0), 'num'),
      td(p.playtime ? duration(p.playtime) : '–', 'num'),
      td(rankPicker(p)),
    );
    return tr;
  }));
}

function rankPicker(player) {
  const select = el('select');
  select.appendChild(new Option('–', ''));
  for (const [key, label] of Object.entries(RANK_LABELS)) select.appendChild(new Option(label, key));
  select.value = player.rank || '';
  select.disabled = !player.name;
  if (!player.name) select.title = 'Name noch unbekannt';
  select.addEventListener('change', async () => {
    const next = select.value;
    select.disabled = true;
    try {
      if (next) {
        await api('/admin/ranks', { action: 'add', rank: next, name: player.name });
        toast(`${player.name} ist jetzt ${RANK_LABELS[next]}.`);
      } else if (player.rank) {
        await api('/admin/ranks', { action: 'remove', rank: player.rank, uuid: player.uuid });
        toast(`${player.name} hat keinen Rang mehr.`);
      }
      player.rank = next;
    } catch (error) {
      toast(error.message, true);
      select.value = player.rank || '';
    } finally {
      select.disabled = false;
    }
  });
  return select;
}

$('dir-q').addEventListener('input', debounce(() => run(loadDirectory), 250));
$('dir-csv').addEventListener('click', () => {
  csv(`mangoclient-spieler-${new Date().toISOString().slice(0, 10)}.csv`,
    ['Name', 'UUID', 'Rang', 'Online', 'Letzter Server', 'Minecraft', 'MangoConfig', 'Zuerst gesehen', 'Zuletzt gesehen', 'Sessions', 'Spielzeit (Min.)'],
    dirRows.map((p) => [p.name, dashed(p.uuid), p.rank, p.online ? 'ja' : 'nein', p.server, p.mc, p.mod, p.first, p.last, p.sessions || 0, Math.round((p.playtime || 0) / 60)]));
});

// --- launcher installs -------------------------------------------------------------

async function loadInstalls() {
  const data = await api('/admin/installs');
  installRows = data.installs;
  renderInstalls();
}

function filteredInstalls() {
  const mode = $('inst-filter').value;
  const month = Date.now() - 30 * 86400 * 1000;
  return installRows.filter((i) => mode === 'all'
    || (mode === 'online' && i.online)
    || (mode === 'month' && new Date(i.last).getTime() > month));
}

function renderInstalls() {
  const rows = filteredInstalls();
  $('inst-count').textContent = `${nf.format(rows.length)} von ${nf.format(installRows.length)}`;
  const tbody = $('inst-table').querySelector('tbody');
  tbody.replaceChildren(...rows.slice(0, 2000).map((i) => {
    const tr = el('tr');
    const id = el('code', null, i.id.slice(0, 8) + '…');
    id.title = i.id;
    tr.append(
      td(id),
      td(i.online ? el('span', 'tag on', 'offen') : el('span', 'tag off', 'zu')),
      td(el('span', 'tag', i.version || '?')),
      td(OS_LABELS[i.os] || i.os || '?'),
      td(dateFmt.format(new Date(i.first))),
      td(ago(i.last)),
      td(nf.format(i.sessions || 0), 'num'),
    );
    return tr;
  }));
}

$('inst-filter').addEventListener('change', renderInstalls);
$('inst-csv').addEventListener('click', () => {
  csv(`mangoclient-installs-${new Date().toISOString().slice(0, 10)}.csv`,
    ['Install-ID', 'Offen', 'Version', 'OS', 'Erste Öffnung', 'Zuletzt offen', 'Sessions'],
    filteredInstalls().map((i) => [i.id, i.online ? 'ja' : 'nein', i.version, i.os, i.first, i.last, i.sessions || 0]));
});

// --- ranks ---------------------------------------------------------------------

async function loadRanks() {
  const data = await api('/admin/ranks');
  const host = $('ranks');
  const frag = document.createDocumentFragment();

  for (const rank of data.order) {
    const list = data.ranks[rank] || [];
    const section = el('section', 'rank');
    section.style.setProperty('--rank-color', RANK_COLORS[rank] || 'var(--member)');

    const header = el('header');
    header.append(el('h3', null, data.labels[rank]), el('span', 'count', String(list.length)), el('span', 'rule'));
    section.appendChild(header);

    const form = el('form', 'adder');
    const input = el('input');
    input.type = 'text';
    input.placeholder = 'Minecraft-Name';
    input.maxLength = 16;
    input.autocomplete = 'off';
    input.spellcheck = false;
    const button = el('button', null, 'Hinzufügen');
    button.type = 'submit';
    form.append(input, button);
    form.addEventListener('submit', (event) => {
      event.preventDefault();
      addRank(rank, input, button);
    });
    section.appendChild(form);

    if (list.length === 0) {
      section.appendChild(el('p', 'empty', 'Noch niemand.'));
    } else {
      const ul = el('ul', 'people');
      for (const member of list) ul.appendChild(rankRow(member, rank));
      section.appendChild(ul);
    }
    frag.appendChild(section);
  }
  host.replaceChildren(frag);
}

function rankRow(member, rank) {
  const li = el('li', 'person');
  li.style.setProperty('--rank-color', RANK_COLORS[rank] || 'var(--member)');
  const img = head(member.uuid);
  img.className = 'head';
  li.appendChild(img);

  const who = el('div', 'who');
  who.append(el('span', 'ign', member.name), el('span', 'since', 'seit ' + dateFmt.format(new Date(member.added))));
  li.appendChild(who);

  const kill = el('button', 'kill', '×');
  kill.type = 'button';
  kill.title = `${member.name} entfernen`;
  kill.setAttribute('aria-label', kill.title);
  kill.addEventListener('click', () => removeRank(rank, member));
  li.appendChild(kill);
  return li;
}

async function addRank(rank, input, button) {
  const name = input.value.trim();
  if (!name) return;
  button.disabled = true;
  toast('Frage Mojang nach ' + name + '…');
  try {
    const data = await api('/admin/ranks', { action: 'add', rank, name });
    input.value = '';
    toast(`${data.member.name} ist jetzt ${RANK_LABELS[rank] || rank}.`);
    await loadRanks();
  } catch (error) {
    toast(error.message, true);
  } finally {
    button.disabled = false;
  }
}

async function removeRank(rank, member) {
  if (!confirm(`${member.name} entfernen?`)) return;
  try {
    await api('/admin/ranks', { action: 'remove', rank, uuid: member.uuid });
    toast(`${member.name} entfernt.`);
    await loadRanks();
  } catch (error) {
    toast(error.message, true);
  }
}

// --- system --------------------------------------------------------------------

async function loadSystem() {
  const s = await api('/admin/system');
  const tiles = [
    ['Laufzeit', duration(s.uptime), 'seit ' + dateTimeFmt.format(new Date(s.startedAt))],
    ['Speicher (Heap)', `${s.heapMB.toFixed(1)} MB`, `${s.sysMB.toFixed(1)} MB vom System`],
    ['Goroutinen', nf.format(s.goroutines), `${nf.format(s.gcRuns)} GC-Läufe`],
    ['Admin-Sitzungen', nf.format(s.adminSessions), 'aktiv'],
    ['Go', s.go, s.domain],
  ];
  $('sys-tiles').replaceChildren(...tiles.map(([label, value, foot]) => {
    const tile = el('div', 'tile');
    tile.append(el('span', 'label', label), el('span', 'value', value), el('span', 'foot', foot));
    return tile;
  }));
}

boot();
