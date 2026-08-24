'use strict';
/*
 * The live numbers.
 *
 * Everything here is a poll of /v1/stats and /v1/ranks - the page holds no
 * state worth reloading for, so a failed poll keeps the last good numbers on
 * screen and only the heartbeat in the corner turns red.
 */

const POLL_MS = 10_000;

const OS_NAMES = { win32: 'Windows', darwin: 'macOS', linux: 'Linux' };
const OS_COLORS = { Windows: '#4ea3ff', macOS: '#c9c1b5', Linux: '#f6a93c', Other: '#7c7367' };

const RANK_COLORS = { owner: 'var(--owner)', support: 'var(--support)', mangoplus: 'var(--mangoplus)' };
const RANK_BLURB = {
  owner: 'Runs MangoClient.',
  support: 'Ask them when something breaks.',
  mangoplus: 'Supporters of the client.',
};

const fmt = new Intl.NumberFormat();
const shown = new Map(); // element id -> the number currently on screen

/** Count from what is on screen to the new value, then flash if it moved. */
function setNumber(id, value) {
  const el = document.getElementById(id);
  if (!el) return;
  const from = shown.has(id) ? shown.get(id) : null;
  shown.set(id, value);
  if (from === value) return;

  if (from === null) {
    // First paint: land on the number, do not count up from zero on a reload.
    el.textContent = fmt.format(value);
    return;
  }

  el.classList.remove('bumped');
  void el.offsetWidth;
  el.classList.add('bumped');

  const start = performance.now();
  const span = 700;
  const step = (now) => {
    const t = Math.min(1, (now - start) / span);
    const eased = 1 - Math.pow(1 - t, 4);
    el.textContent = fmt.format(Math.round(from + (value - from) * eased));
    if (t < 1) requestAnimationFrame(step);
  };
  requestAnimationFrame(step);
}

function word(id, text) {
  const el = document.getElementById(id);
  if (el) el.textContent = text;
}

function pulse(ok) {
  const el = document.getElementById('pulse');
  if (!el) return;
  el.classList.toggle('stale', !ok);
  el.textContent = ok ? 'live' : 'offline';
}

// --- stats -------------------------------------------------------------------

async function loadStats() {
  const stats = await fetch('/v1/stats', { cache: 'no-store' }).then((r) => r.json());

  setNumber('installs', stats.installs);
  setNumber('open', stats.launchersOpen);
  setNumber('ingame', stats.inGame);
  setNumber('active', stats.activeMonth);
  setNumber('sessions', stats.sessions);
  setNumber('players', stats.playersEver);

  // The sentence has to stay true at one, at zero, and at ten thousand.
  word('installs-word', stats.installs === 1
    ? 'person has installed MangoClient.'
    : 'people have installed MangoClient.');
  word('open-word', stats.launchersOpen === 1 ? 'has it open right now' : 'have it open right now');
  word('ingame-word', stats.inGame === 1 ? 'of them in a world' : 'of them in a world');

  drawSplit(stats.byOS);
}

function drawSplit(byOS) {
  const bar = document.getElementById('os-bar');
  const keys = document.getElementById('os-keys');
  if (!bar || !keys) return;

  const counted = {};
  let total = 0;
  for (const [raw, n] of Object.entries(byOS || {})) {
    const name = OS_NAMES[raw] || 'Other';
    counted[name] = (counted[name] || 0) + n;
    total += n;
  }
  if (!total) {
    bar.innerHTML = '';
    keys.innerHTML = '<span style="color:var(--text-4)">nothing reported yet</span>';
    return;
  }

  const rows = Object.entries(counted).sort((a, b) => b[1] - a[1]);
  bar.innerHTML = rows
    .map(([name, n]) => `<span style="width:${(n / total) * 100}%;background:${OS_COLORS[name] || OS_COLORS.Other}"></span>`)
    .join('');
  keys.innerHTML = rows
    .map(([name, n]) => `<span><i style="background:${OS_COLORS[name] || OS_COLORS.Other}"></i><b>${name}</b> ${Math.round((n / total) * 100)}%</span>`)
    .join('');
}

// --- staff -------------------------------------------------------------------

function head(member) {
  const img = document.createElement('img');
  img.className = 'head';
  img.loading = 'lazy';
  img.alt = '';
  img.src = `https://mc-heads.net/avatar/${member.uuid}/68`;
  img.onerror = () => { img.style.visibility = 'hidden'; };
  return img;
}

function personRow(member, rank) {
  const li = document.createElement('li');
  li.className = 'person';
  li.style.setProperty('--rank-color', RANK_COLORS[rank] || 'var(--member)');
  li.appendChild(head(member));

  const who = document.createElement('div');
  who.className = 'who';
  const ign = document.createElement('span');
  ign.className = 'ign';
  ign.textContent = member.name;
  who.appendChild(ign);
  li.appendChild(who);

  const mango = document.createElement('img');
  mango.className = 'mango';
  mango.alt = '';
  mango.src = `mango-${rank}.png`;
  mango.title = `${member.name} carries this mango in game`;
  li.appendChild(mango);
  return li;
}

async function loadRanks() {
  const data = await fetch('/v1/ranks', { cache: 'no-store' }).then((r) => r.json());
  const host = document.getElementById('ranks');
  if (!host) return;

  const frag = document.createDocumentFragment();
  for (const rank of data.order) {
    const list = data.ranks[rank] || [];
    const section = document.createElement('section');
    section.className = 'rank';
    section.style.setProperty('--rank-color', RANK_COLORS[rank] || 'var(--member)');

    const header = document.createElement('header');
    header.innerHTML = `<h3>${data.labels[rank]}</h3>`
      + `<span class="count">${list.length}</span>`
      + '<span class="rule"></span>';
    section.appendChild(header);

    const blurb = document.createElement("p");
    blurb.className = "blurb";
    blurb.textContent = RANK_BLURB[rank] || "";
    section.appendChild(blurb);

    if (list.length === 0) {
      const empty = document.createElement("p");
      empty.className = "empty";
      empty.textContent = "Nobody yet.";
      section.appendChild(empty);
    } else {
      const ul = document.createElement('ul');
      ul.className = 'people';
      for (const member of list) ul.appendChild(personRow(member, rank));
      section.appendChild(ul);
    }
    frag.appendChild(section);
  }

  host.replaceChildren(frag);
}

// --- loop --------------------------------------------------------------------

async function tick() {
  try {
    await loadStats();
    pulse(true);
  } catch {
    pulse(false);
  }
}

tick();
loadRanks().catch(() => {});
setInterval(tick, POLL_MS);
// Staff changes rarely; once a minute is plenty, and only while the tab is open.
setInterval(() => { if (!document.hidden) loadRanks().catch(() => {}); }, 60_000);
document.addEventListener('visibilitychange', () => { if (!document.hidden) tick(); });
