'use strict';
/*
 * The staff editor.
 *
 * Type an IGN, press add: the server resolves it against Mojang before it
 * saves, so a typo comes back as an error here instead of quietly never
 * matching anyone in game. Ranks are stored by uuid, so a name change keeps
 * the colour.
 */

const RANK_COLORS = { owner: 'var(--owner)', support: 'var(--support)', mangoplus: 'var(--mangoplus)' };

const gate = document.getElementById('gate');
const panel = document.getElementById('panel');
const note = document.getElementById('note');

function say(el, message, kind) {
  el.textContent = message || '';
  el.className = 'note' + (kind ? ' ' + kind : '');
}

async function api(path, body) {
  const options = body
    ? { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(body) }
    : { cache: 'no-store' };
  const response = await fetch(path, options);
  const data = await response.json().catch(() => ({}));
  if (!response.ok) throw new Error(data.error || `Something went wrong (${response.status})`);
  return data;
}

// --- the gate ----------------------------------------------------------------

async function boot() {
  const session = await api('/admin/session').catch(() => ({ authed: false }));
  if (session.authed) {
    show(true);
    load();
  } else {
    show(false);
    if (session.configured === false) {
      say(document.getElementById('login-note'),
        'No password is set on the server yet. Set MANGO_ADMIN_PASSWORD and restart the service.', 'bad');
    }
  }
}

function show(authed) {
  gate.hidden = authed;
  panel.hidden = !authed;
  if (!authed) document.getElementById('password').focus();
}

document.getElementById('login').addEventListener('submit', async (event) => {
  event.preventDefault();
  const field = document.getElementById('password');
  const noteEl = document.getElementById('login-note');
  say(noteEl, 'Checking…');
  try {
    await api('/admin/login', { password: field.value });
    field.value = '';
    say(noteEl, '');
    show(true);
    load();
  } catch (error) {
    say(noteEl, error.message, 'bad');
    field.select();
  }
});

document.getElementById('logout').addEventListener('click', async () => {
  await api('/admin/logout', {}).catch(() => {});
  show(false);
});

// --- the lists ---------------------------------------------------------------

async function load() {
  const data = await api('/admin/ranks');
  const host = document.getElementById('ranks');
  const frag = document.createDocumentFragment();

  for (const rank of data.order) {
    const list = data.ranks[rank] || [];
    const section = document.createElement('section');
    section.className = 'rank';
    section.style.setProperty('--rank-color', RANK_COLORS[rank] || 'var(--member)');

    const header = document.createElement('header');
    header.innerHTML = `<h3>${data.labels[rank]}</h3><span class="count">${list.length}</span><span class="rule"></span>`;
    section.appendChild(header);

    const form = document.createElement('form');
    form.className = 'adder';
    form.innerHTML = '<input type="text" placeholder="Minecraft name" maxlength="16" autocomplete="off" spellcheck="false">'
      + '<button type="submit">Add</button>';
    form.addEventListener('submit', (event) => {
      event.preventDefault();
      add(rank, form);
    });
    section.appendChild(form);

    if (list.length === 0) {
      const empty = document.createElement('p');
      empty.className = 'empty';
      empty.textContent = 'Nobody yet.';
      section.appendChild(empty);
    } else {
      const ul = document.createElement('ul');
      ul.className = 'people';
      for (const member of list) ul.appendChild(row(member, rank));
      section.appendChild(ul);
    }
    frag.appendChild(section);
  }

  host.replaceChildren(frag);
}

function row(member, rank) {
  const li = document.createElement('li');
  li.className = 'person';
  li.style.setProperty('--rank-color', RANK_COLORS[rank] || 'var(--member)');

  const img = document.createElement('img');
  img.className = 'head';
  img.alt = '';
  img.loading = 'lazy';
  img.src = `https://mc-heads.net/avatar/${member.uuid}/68`;
  img.onerror = () => { img.style.visibility = 'hidden'; };
  li.appendChild(img);

  const who = document.createElement('div');
  who.className = 'who';
  const ign = document.createElement('span');
  ign.className = 'ign';
  ign.textContent = member.name;
  const since = document.createElement('span');
  since.className = 'since';
  since.textContent = 'added ' + new Date(member.added).toLocaleDateString();
  who.append(ign, since);
  li.appendChild(who);

  const kill = document.createElement('button');
  kill.className = 'kill';
  kill.type = 'button';
  kill.textContent = '×';
  kill.title = `Remove ${member.name}`;
  kill.setAttribute('aria-label', `Remove ${member.name}`);
  kill.addEventListener('click', () => remove(rank, member));
  li.appendChild(kill);
  return li;
}

async function add(rank, form) {
  const input = form.querySelector('input');
  const button = form.querySelector('button');
  const name = input.value.trim();
  if (!name) return;

  button.disabled = true;
  say(note, 'Asking Mojang about ' + name + '…');
  try {
    const data = await api('/admin/ranks', { action: 'add', rank, name });
    input.value = '';
    say(note, `${data.member.name} is now ${rank === 'mangoplus' ? 'Mango+' : rank}.`, 'good');
    await load();
  } catch (error) {
    say(note, error.message, 'bad');
  } finally {
    button.disabled = false;
  }
}

async function remove(rank, member) {
  if (!confirm(`Remove ${member.name}?`)) return;
  try {
    await api('/admin/ranks', { action: 'remove', rank, uuid: member.uuid });
    say(note, `${member.name} removed.`, 'good');
    await load();
  } catch (error) {
    say(note, error.message, 'bad');
  }
}

boot();
