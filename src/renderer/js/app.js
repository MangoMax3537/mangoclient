/* MangoClient renderer */
(function () {
  'use strict';

  const { t } = window.i18n;
  const { icon, logoMark, tileColor, TILE_COLORS } = window.icons;
  const api = window.mango;

  const $ = (sel, root = document) => root.querySelector(sel);
  const $$ = (sel, root = document) => [...root.querySelectorAll(sel)];

  const state = {
    config: {},
    profiles: [],
    accounts: [],
    profile: null,
    account: null,
    totalRam: 8192,
    running: new Set(),
    launchState: 'idle',
    versions: { versions: [], latest: {}, installed: [] },
    servers: [],
    skins: new Map(),
    covers: new Map(),
    modSearch: { offset: 0 },
    consoleLines: [],
  };

  let viewer = null;

  // =========================================================================
  // helpers
  // =========================================================================

  function esc(str) {
    return String(str ?? '').replace(/[&<>"']/g, (c) =>
      ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));
  }

  /** Replace every <span data-icon="name"> placeholder with its SVG. */
  function hydrateIcons(root = document) {
    $$('[data-icon]', root).forEach((el) => {
      if (el.dataset.iconDone) return;
      el.dataset.iconDone = '1';
      el.insertAdjacentHTML('afterbegin', icon(el.dataset.icon));
    });
  }

  function toast(message, kind = 'info', title) {
    const el = document.createElement('div');
    el.className = `toast ${kind}`;
    const glyph = kind === 'err' ? 'alert' : kind === 'ok' ? 'check' : 'info';
    const heading = title || (kind === 'err' ? t('toast.error') : kind === 'ok' ? t('toast.success') : t('toast.info'));
    el.innerHTML = `${icon(glyph)}<div><div class="tt">${esc(heading)}</div><div class="td">${esc(message)}</div></div>`;
    $('#toasts').appendChild(el);
    setTimeout(() => {
      el.style.transition = 'opacity .2s';
      el.style.opacity = '0';
      setTimeout(() => el.remove(), 220);
    }, kind === 'err' ? 7000 : 4000);
  }

  function fmtNumber(n) {
    if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
    if (n >= 1000) return `${(n / 1000).toFixed(1)}k`;
    return String(n ?? 0);
  }

  function fmtDuration(ms) {
    if (!ms) return '0 h';
    const h = Math.floor(ms / 3600000);
    const m = Math.floor((ms % 3600000) / 60000);
    return h > 0 ? `${h} h ${m} min` : `${m} min`;
  }

  function fmtDate(ts) {
    if (!ts) return t('profiles.never');
    return new Date(ts).toLocaleDateString(window.i18n.lang === 'de' ? 'de-DE' : 'en-GB',
      { day: '2-digit', month: '2-digit', year: '2-digit' });
  }

  function fmtGB(mb) {
    return `${(mb / 1024).toFixed(1)} GB`;
  }

  const LOADER_LABEL = { vanilla: 'Vanilla', fabric: 'Fabric', quilt: 'Quilt', neoforge: 'NeoForge', forge: 'Forge' };

  function javaBadgeFor(mcVersion) {
    // 26.x and later use a year.major scheme and need a much newer JVM.
    if (/^\d\d\./.test(mcVersion)) return 'Java 25+';
    const m = mcVersion.match(/^1\.(\d+)/);
    const minor = m ? Number(m[1]) : 21;
    if (minor >= 20) return 'Java 21+';
    if (minor >= 18) return 'Java 17+';
    if (minor >= 17) return 'Java 16+';
    return 'Java 8+';
  }

  function coverColor(profile) {
    return profile?.color || tileColor(profile?.id || profile?.name || '');
  }

  /**
   * A profile shows its own picture if one was chosen; otherwise it falls back
   * to its initial on a colour derived from its id.
   */
  function coverStyle(profile) {
    const picture = profile && state.covers.get(profile.id);
    return picture
      ? `background-image:url(${picture})`
      : `background:${esc(coverColor(profile))}`;
  }

  function coverLetter(profile) {
    if (profile && state.covers.get(profile.id)) return '';
    return esc((profile?.name || '?').trim().slice(0, 1));
  }

  function coverHtml(profile, size = '') {
    return `<span class="cover ${size}" style="${coverStyle(profile)}">${coverLetter(profile)}</span>`;
  }

  // =========================================================================
  // modal
  // =========================================================================

  function openModal({ title, body, buttons = [], onOpen, onClose }) {
    $('#modal-title').textContent = title;
    $('#modal-body').innerHTML = body;
    const foot = $('#modal-foot');
    foot.innerHTML = '';
    for (const b of buttons) {
      const btn = document.createElement('button');
      btn.className = `btn${b.brand ? ' brand' : ''}${b.danger ? ' danger' : ''}`;
      btn.textContent = b.label;
      btn.onclick = () => b.onClick?.(closeModal);
      if (b.id) btn.id = b.id;
      foot.appendChild(btn);
    }
    modalOnClose = onClose || null;
    $('#modal-backdrop').hidden = false;
    hydrateIcons($('#modal'));
    onOpen?.($('#modal-body'));
  }

  let modalOnClose = null;

  function closeModal() {
    $('#modal-backdrop').hidden = true;
    $('#modal-body').innerHTML = '';
    const fn = modalOnClose;
    modalOnClose = null;
    fn?.();
  }

  $('#modal-close').onclick = () => closeModal();
  $('#modal-backdrop').onclick = (e) => { if (e.target === $('#modal-backdrop')) closeModal(); };
  document.addEventListener('keydown', (e) => {
    if (e.key !== 'Escape') return;
    if (!$('#modal-backdrop').hidden) closeModal();
    else if (!$('#console-drawer').hidden) $('#console-drawer').hidden = true;
  });

  // =========================================================================
  // navigation
  // =========================================================================

  /** Browser-style history so the statusbar's arrows do something real. */
  const history = { stack: ['start'], idx: 0 };

  function renderView(name) {
    $$('.view').forEach((v) => v.classList.toggle('active', v.id === `view-${name}`));
    $$('.rail-btn').forEach((n) => n.classList.toggle('active', n.dataset.view === name));
    $('#breadcrumb').textContent = t(`nav.${name}`);
    $('#nav-back').disabled = history.idx === 0;
    $('#nav-fwd').disabled = history.idx >= history.stack.length - 1;

    if (name === 'mods') renderMods();
    if (name === 'servers') renderServerGrid();
    if (name === 'settings') renderSettings();
    if (name === 'profiles') renderProfiles();
    if (name === 'accounts') renderAccounts();
  }

  function showView(name) {
    if (history.stack[history.idx] === name) return;
    history.stack = history.stack.slice(0, history.idx + 1);
    history.stack.push(name);
    history.idx = history.stack.length - 1;
    renderView(name);
  }

  function goHistory(delta) {
    const next = history.idx + delta;
    if (next < 0 || next >= history.stack.length) return;
    history.idx = next;
    renderView(history.stack[next]);
  }

  $$('.rail-btn[data-view]').forEach((btn) => { btn.onclick = () => showView(btn.dataset.view); });
  $$('[data-view-link]').forEach((btn) => { btn.onclick = () => showView(btn.dataset.viewLink); });
  $('#nav-back').onclick = () => goHistory(-1);
  $('#nav-fwd').onclick = () => goHistory(1);

  $('#btn-min').onclick = () => api.window.minimize();
  $('#btn-max').onclick = () => api.window.maximize();
  $('#btn-close').onclick = () => api.window.close();

  $('#btn-sidebar').onclick = async () => {
    const open = document.body.classList.toggle('sidebar-off') === false;
    state.config = await api.app.setConfig({ sidebarOpen: open });
  };

  $('#rail-new').onclick = () => openProfileDialog(null);
  $('#run-pill').onclick = () => { $('#console-drawer').hidden = false; scrollConsole(); };

  // =========================================================================
  // updates
  // =========================================================================

  function updateStatusText(u) {
    switch (u?.state) {
      case 'checking': return t('update.checking');
      case 'downloading': return t('update.downloading', { version: u.version, percent: u.percent ?? 0 });
      case 'ready': return t('update.ready', { version: u.version });
      case 'error': return t('update.error', { error: u.error });
      case 'disabled': return u.reason === 'dev' ? t('update.disabledDev') : t('update.disabledTarget');
      case 'current': return t('update.current');
      default: return t('update.idle');
    }
  }

  function renderUpdate() {
    const u = state.update;

    // The launch-time update takes over the whole window: it will relaunch on
    // its own, so there is nothing for the player to decide.
    const applying = Boolean(u?.applying) && (u.state === 'downloading' || u.state === 'ready');
    $('#update-overlay').hidden = !applying;
    if (applying) {
      $('#uo-text').textContent = u.state === 'ready'
        ? t('update.restarting')
        : t('update.applyingText', { version: u.version, percent: u.percent ?? 0 });
      $('#uo-fill').style.width = `${u.state === 'ready' ? 100 : (u.percent ?? 0)}%`;
    }

    const pill = $('#update-pill');
    pill.hidden = u?.state !== 'ready' || applying;
    if (!pill.hidden) $('#update-pill-text').textContent = t('update.pill', { version: u.version });
    if ($('#view-settings').classList.contains('active')) renderUpdateCard();
  }

  function renderUpdateCard() {
    const box = $('#update-status');
    if (!box) return;
    const u = state.update || {};
    box.textContent = updateStatusText(u);
    const install = $('#btn-update-install');
    if (install) install.hidden = u.state !== 'ready';
    const check = $('#btn-update-check');
    if (check) check.disabled = u.state === 'checking' || u.state === 'downloading';
  }

  $('#update-pill').onclick = () => api.update.install().catch((err) => toast(err.message, 'err'));

  api.on.updateState((u) => {
    const wasReady = state.update?.state === 'ready';
    state.update = u;
    renderUpdate();
    // No point offering a restart the launcher is about to do anyway.
    if (u.state === 'ready' && !wasReady && !u.applying) {
      toast(t('update.readyToast', { version: u.version }), 'ok', t('update.readyTitle'));
    }
  });

  document.addEventListener('click', () => { $('#account-menu').hidden = true; });

  // =========================================================================
  // start view
  // =========================================================================

  function renderStart() {
    const profile = state.profile;

    const tile = $('#lp-tile');
    tile.setAttribute('style', profile ? coverStyle(profile) : 'background:var(--surface-4)');
    tile.textContent = profile ? coverLetter(profile) : '';
    $('#btn-edit-current').disabled = !profile;

    if (profile) {
      $('#lp-name').textContent = profile.name;
      $('#lp-sub').textContent = `${profile.mcVersion} · ${LOADER_LABEL[profile.loader] || profile.loader}`;

      const ram = profile.ram || state.config.ram || 4096;
      const facts = [
        [t('facts.version'), profile.mcVersion],
        [t('facts.loader'), profile.loaderVersion
          ? `${LOADER_LABEL[profile.loader] || profile.loader} ${profile.loaderVersion}`
          : (LOADER_LABEL[profile.loader] || profile.loader)],
        [t('facts.java'), javaBadgeFor(profile.mcVersion)],
        [t('facts.ram'), fmtGB(ram)],
        [t('facts.mods'), String((profile.mods || []).length)],
        [t('facts.lastPlayed'), fmtDate(profile.lastPlayed)],
      ];
      $('#lp-facts').innerHTML = facts
        .map(([k, v]) => `<div class="fact"><span class="k">${esc(k)}</span><span class="v">${esc(v)}</span></div>`)
        .join('');
      $('#ram-warn').hidden = ram >= 3072;
    } else {
      $('#lp-name').textContent = t('profiles.none');
      $('#lp-sub').textContent = '';
      $('#lp-facts').innerHTML = '';
      $('#ram-warn').hidden = true;
    }

    updatePlayButton();
    renderAccountChip();
    renderRunPill();
    renderRailInstances();
    renderModList($('#start-mod-list'));
    $('#start-mod-count').textContent = (state.profile?.mods || []).length || '';
  }

  /** Rail quick-switcher: most recently played instance sits at the top. */
  function renderRailInstances() {
    const rail = $('#rail-instances');
    const ordered = [...state.profiles].sort((a, b) => (b.lastPlayed || 0) - (a.lastPlayed || 0));

    rail.innerHTML = ordered.map((p) => `
      <button class="rail-instance ${p.id === state.profile?.id ? 'active' : ''}"
              data-id="${esc(p.id)}" data-tip-text="${esc(p.name)}">
        ${coverHtml(p)}
        ${state.running.has(p.id) ? '<span class="running-dot"></span>' : ''}
      </button>`).join('');

    $$('.rail-instance', rail).forEach((btn) => {
      btn.onclick = async () => {
        await api.profiles.select(btn.dataset.id);
        await refreshState();
        showView('start');
      };
    });
  }

  function updatePlayButton() {
    const btn = $('#btn-play');
    const label = $('#play-label');
    const running = state.profile && state.running.has(state.profile.id);
    const busy = ['preparing', 'java', 'loader', 'downloading', 'account', 'launching'].includes(state.launchState);

    btn.disabled = busy;
    btn.classList.toggle('brand', !running);
    label.textContent = running ? t('btn.stop') : busy ? t('btn.preparing') : t('btn.play');

    const glyph = $('.ic', btn);
    if (glyph) glyph.outerHTML = icon(running ? 'stop' : 'play');
  }

  /** Running instances live in the statusbar, next to the console button. */
  function renderRunPill() {
    const running = [...state.running];
    const pill = $('#run-pill');
    pill.hidden = running.length === 0;
    if (!running.length) return;
    const names = running.map((id) => state.profiles.find((p) => p.id === id)?.name || id);
    $('#run-name').textContent = names.join(', ');
  }

  function accountTypeLabel(account) {
    if (!account) return t('account.signInHint');
    if (account.type === 'offline') return t('account.offline');
    return account.expired ? t('accounts.expired') : 'Microsoft';
  }

  function renderAccountChip() {
    const account = state.account;
    const head = (account && state.skins.get(account.uuid)?.head) || state.defaultHead;

    const avatar = $('#chip-avatar');
    if (head) avatar.src = head;
    else avatar.removeAttribute('src');
    $('#account-chip').dataset.tipText = account ? account.name : t('account.none');

    $('#side-account').innerHTML = `
      <div class="sa-row">
        <img src="${head || ''}" alt="" />
        <div style="min-width:0">
          <div class="sa-name">${esc(account ? account.name : t('account.none'))}</div>
          <div class="sa-sub">${esc(accountTypeLabel(account))}</div>
        </div>
      </div>
      ${account ? '' : `<button class="btn brand" data-side-signin>${esc(t('accounts.addMs'))}</button>`}`;
    const signIn = $('[data-side-signin]');
    if (signIn) signIn.onclick = () => $('#btn-add-msa').click();
  }

  $('#account-chip').onclick = (e) => {
    e.stopPropagation();
    const menu = $('#account-menu');
    if (!menu.hidden) { menu.hidden = true; return; }

    const rows = state.accounts.map((a) => {
      const skin = state.skins.get(a.uuid);
      return `<button data-uuid="${esc(a.uuid)}" class="${a.uuid === state.account?.uuid ? 'active' : ''}">
        <img src="${skin?.head || ''}" alt="" />
        <span><span>${esc(a.name)}</span><br><span class="mi-sub">${a.type === 'offline' ? esc(t('account.offline')) : 'Microsoft'}</span></span>
      </button>`;
    }).join('');
    menu.innerHTML = `${rows}${rows ? '<div class="sep"></div>' : ''}
      <button data-action="manage" data-icon="user">${esc(t('nav.accounts'))}</button>`;
    hydrateIcons(menu);

    $$('button[data-uuid]', menu).forEach((b) => {
      b.onclick = async () => {
        await api.auth.select(b.dataset.uuid);
        await refreshState();
        menu.hidden = true;
      };
    });
    $('button[data-action="manage"]', menu).onclick = () => { menu.hidden = true; showView('accounts'); };
    menu.hidden = false;
  };
  $('#account-menu').onclick = (e) => e.stopPropagation();

  $('#btn-edit-current').onclick = () => state.profile && openProfileDialog(state.profile);
  $('#btn-start-add-mods').onclick = () => showView('mods');
  $('#btn-start-mods-folder').onclick = () => state.profile && api.app.openFolder(state.profile.id);
  $('#btn-console').onclick = () => { $('#console-drawer').hidden = false; scrollConsole(); };
  $('#btn-close-console').onclick = () => { $('#console-drawer').hidden = true; };
  $('#btn-clear-console').onclick = () => { state.consoleLines = []; $('#console-out').innerHTML = ''; };
  $('#btn-kill').onclick = async () => {
    for (const id of state.running) await api.game.stop(id).catch(() => {});
  };

  $('#btn-play').onclick = async () => {
    if (!state.profile) { showView('profiles'); return; }
    if (state.running.has(state.profile.id)) {
      await api.game.stop(state.profile.id).catch(() => {});
      return;
    }
    if (!state.account) { toast(t('launch.needAccount'), 'err'); showView('accounts'); return; }
    await doLaunch(state.profile.id);
  };

  async function doLaunch(profileId, quickJoin) {
    state.launchState = 'preparing';
    updatePlayButton();
    $('#launch-progress').hidden = false;
    setProgress(0, t('launch.preparing'));
    try {
      await api.game.launch(profileId, quickJoin);
    } catch (err) {
      state.launchState = 'idle';
      $('#launch-progress').hidden = true;
      updatePlayButton();
      toast(err.message, 'err');
      if (err.needsRelogin) showView('accounts');
      $('#console-drawer').hidden = false;
      pushConsole(`Launch failed: ${err.message}`, 'error');
    }
  }

  function setProgress(pct, label) {
    $('#progress-fill').style.width = `${Math.max(0, Math.min(100, pct))}%`;
    $('#progress-pct').textContent = `${Math.round(pct)}%`;
    $('#progress-text').textContent = label;
  }

  // =========================================================================
  // skin viewer
  // =========================================================================

  function ensureViewer() {
    if (viewer) return viewer;
    try {
      viewer = new window.SkinViewer($('#skin-canvas'), { slim: false });
    } catch (err) {
      console.warn('SkinViewer unavailable:', err);
      $('#skin-canvas').replaceWith(Object.assign(document.createElement('div'), {
        className: 'empty', textContent: t('skin.noWebgl'),
      }));
    }
    return viewer;
  }

  /** Crop the 8x8 face (+ hat overlay) out of a skin for list avatars. */
  function makeHead(skinDataUrl) {
    return new Promise((resolve) => {
      const img = new Image();
      img.onload = () => {
        const c = document.createElement('canvas');
        c.width = 64; c.height = 64;
        const ctx = c.getContext('2d');
        ctx.imageSmoothingEnabled = false;
        ctx.drawImage(img, 8, 8, 8, 8, 0, 0, 64, 64);
        ctx.drawImage(img, 40, 8, 8, 8, 0, 0, 64, 64);
        resolve(c.toDataURL('image/png'));
      };
      img.onerror = () => resolve(null);
      img.src = skinDataUrl;
    });
  }

  /** Show the fallback model so the stage is never an empty box. */
  async function loadDefaultSkin() {
    try {
      const skin = await api.skins.default();
      state.defaultHead = await makeHead(skin.skin);
      if (!state.account) {
        ensureViewer()?.setSkin(skin.skin, false);
        renderAccountChip();
      }
    } catch { /* WebGL or IPC unavailable, the stage just stays empty */ }
  }

  async function loadSkin(uuid) {
    if (!uuid) return;
    try {
      await applySkin(uuid, await api.skins.get(uuid));
    } catch (err) {
      console.warn('skin load failed', err);
    }
  }

  async function applySkin(uuid, skin) {
    skin.head = await makeHead(skin.skin);
    state.skins.set(uuid, skin);
    if (state.account?.uuid === uuid) ensureViewer()?.setSkin(skin.skin, skin.slim);
    renderAccountChip();
    if ($('#view-accounts').classList.contains('active')) renderAccounts();
  }

  // =========================================================================
  // profiles
  // =========================================================================

  function renderProfiles() {
    const grid = $('#profile-grid');
    if (!state.profiles.length) {
      grid.innerHTML = `<div class="empty"><div class="et">${esc(t('profiles.none'))}</div>${esc(t('profiles.noneHint'))}</div>`;
      return;
    }
    grid.innerHTML = state.profiles.map((p) => {
      const selected = p.id === state.profile?.id;
      const running = state.running.has(p.id);
      return `<div class="instance-card ${selected ? 'selected' : ''}" data-id="${esc(p.id)}">
        <div class="instance-cover" style="${coverStyle(p)}">
          ${coverLetter(p)}
          ${running ? `<span class="instance-running"><span class="dot up"></span>${esc(t('status.runningShort'))}</span>` : ''}
          <div class="instance-overlay">
            <button class="instance-play" data-act="play" data-icon="${running ? 'stop' : 'play'}"
                    title="${esc(running ? t('btn.stop') : t('profiles.play'))}"></button>
            <button class="instance-more" data-act="menu" data-icon="sliders" title="${esc(t('profiles.more'))}"></button>
          </div>
        </div>
        <div class="instance-name">${esc(p.name)}</div>
        <div class="instance-sub">${esc(p.mcVersion)} · ${esc(LOADER_LABEL[p.loader] || p.loader)}</div>
      </div>`;
    }).join('');
    hydrateIcons(grid);

    $$('.instance-card', grid).forEach((card) => {
      const id = card.dataset.id;
      // Clicking the tile itself selects the instance and shows it on Start.
      card.onclick = async (e) => {
        if (e.target.closest('[data-act]')) return;
        await api.profiles.select(id);
        await refreshState();
        showView('start');
      };
      $('[data-act="menu"]', card).onclick = (e) => {
        e.stopPropagation();
        openProfileMenu(state.profiles.find((p) => p.id === id));
      };
      $('[data-act="play"]', card).onclick = async (e) => {
        e.stopPropagation();
        if (state.running.has(id)) { await api.game.stop(id).catch(() => {}); return; }
        await api.profiles.select(id);
        await refreshState();
        showView('start');
        await doLaunch(id);
      };
    });
  }

  /** Per-instance actions, reached from the tile's overlay button. */
  function openProfileMenu(profile) {
    if (!profile) return;
    const actions = [
      { key: 'edit', label: t('profiles.edit'), glyph: 'pencil' },
      { key: 'dup', label: t('profiles.duplicate'), glyph: 'copy' },
      { key: 'folder', label: t('profiles.folder'), glyph: 'folder' },
      { key: 'mods', label: t('profiles.manageMods'), glyph: 'package' },
      { key: 'del', label: t('btn.delete'), glyph: 'trash', danger: true },
    ];
    openModal({
      title: profile.name,
      body: `<div class="rows">${actions.map((a) =>
        `<button class="row menu-row${a.danger ? ' danger' : ''}" data-act="${a.key}">
           <span data-icon="${a.glyph}"></span><span class="row-name">${esc(a.label)}</span>
         </button>`).join('')}</div>`,
      buttons: [{ label: t('btn.close'), onClick: (close) => close() }],
      onOpen: (body) => {
        hydrateIcons(body);
        $$('[data-act]', body).forEach((btn) => {
          btn.onclick = async () => {
            const act = btn.dataset.act;
            if (act === 'del') {
              closeModal();
              confirmDialog({
                title: t('profiles.deleteTitle'),
                text: t('profiles.deleteConfirm', { name: profile.name }),
                confirmLabel: t('btn.delete'),
                onConfirm: async () => { await api.profiles.remove(profile.id); await refreshState(); },
              });
              return;
            }
            if (act === 'edit') { closeModal(); openProfileDialog(profile); return; }
            closeModal();
            if (act === 'dup') { await api.profiles.duplicate(profile.id); await refreshState(); }
            if (act === 'folder') await api.app.openFolder(profile.id);
            if (act === 'mods') {
              await api.profiles.select(profile.id);
              await refreshState();
              showView('mods');
            }
          };
        });
      },
    });
  }

  function confirmDialog({ title, text, confirmLabel, onConfirm }) {
    openModal({
      title,
      body: `<p>${esc(text)}</p>`,
      buttons: [
        { label: t('btn.cancel'), onClick: (close) => close() },
        { label: confirmLabel, danger: true, onClick: async (close) => { close(); await onConfirm(); } },
      ],
    });
  }

  $('#btn-new-profile').onclick = () => openProfileDialog(null);

  async function openProfileDialog(profile) {
    const isNew = !profile;
    const data = profile || {
      name: t('profiles.defaultName'),
      mcVersion: state.versions.latest?.release || '1.21.11',
      loader: 'fabric',
      loaderVersion: '',
      color: TILE_COLORS[0],
      ram: null,
    };
    const activeColor = data.color || tileColor(data.id || data.name);

    const versions = state.versions.versions
      .filter((v) => state.config.showSnapshots || v.type === 'release')
      .slice(0, 300);

    openModal({
      title: isNew ? t('profiles.new') : t('profiles.edit'),
      body: `
        <div class="field">
          <label>${esc(t('profiles.name'))}</label>
          <input id="pf-name" value="${esc(data.name)}" />
        </div>
        <div class="field">
          <label>${esc(t('profiles.picture'))}</label>
          <div class="cover-picker">
            <span class="cover" id="pf-preview" style="${isNew ? `background:${esc(activeColor)}` : coverStyle(data)}">${isNew ? esc(data.name.trim().slice(0, 1)) : coverLetter(data)}</span>
            <div class="cover-picker-actions">
              <button class="btn sm" id="pf-pick-cover" data-icon="download"${isNew ? ' disabled' : ''}>
                <span>${esc(t('profiles.chooseImage'))}</span>
              </button>
              <button class="btn sm danger" id="pf-clear-cover"${isNew || !data.cover ? ' hidden' : ''}>${esc(t('profiles.removeImage'))}</button>
              <div class="hint">${esc(isNew ? t('profiles.pictureNewHint') : t('profiles.pictureHint'))}</div>
            </div>
          </div>
        </div>
        <div class="field" id="pf-color-field">
          <label>${esc(t('profiles.color'))}</label>
          <div class="swatches" id="pf-colors">
            ${TILE_COLORS.map((c) => `<button class="swatch${c === activeColor ? ' active' : ''}" data-color="${c}" style="background:${c}" aria-label="${c}"></button>`).join('')}
          </div>
        </div>
        <div class="field">
          <label>${esc(t('profiles.version'))}</label>
          <select id="pf-version">
            ${versions.map((v) => `<option value="${esc(v.id)}"${v.id === data.mcVersion ? ' selected' : ''}>${esc(v.id)}${v.type !== 'release' ? ` (${esc(v.type)})` : ''}</option>`).join('')}
          </select>
        </div>
        <div class="field">
          <label>${esc(t('profiles.loader'))}</label>
          <select id="pf-loader">
            ${['vanilla', 'fabric', 'quilt', 'neoforge', 'forge'].map((l) =>
              `<option value="${l}"${l === data.loader ? ' selected' : ''}>${LOADER_LABEL[l]}</option>`).join('')}
          </select>
        </div>
        <div class="field" id="pf-loaderver-field">
          <label>${esc(t('profiles.loaderVersion'))}</label>
          <select id="pf-loaderver"><option value="">${esc(t('profiles.latest'))}</option></select>
        </div>
        <div class="field">
          <div class="range-head">
            <label>${esc(t('profiles.ram'))}</label>
            <b id="pf-ram-label">${data.ram ? esc(fmtGB(data.ram)) : esc(t('profiles.ramInherit'))}</b>
          </div>
          <input type="range" id="pf-ram" min="0" max="${Math.max(4096, state.totalRam)}" step="512" value="${data.ram || 0}" />
          <div class="hint">${esc(t('profiles.ramHint'))}</div>
        </div>`,
      buttons: [
        { label: t('btn.cancel'), onClick: (close) => close() },
        {
          label: isNew ? t('btn.create') : t('btn.save'),
          brand: true,
          onClick: async (close) => {
            const patch = {
              name: $('#pf-name').value.trim() || t('profiles.defaultName'),
              mcVersion: $('#pf-version').value,
              loader: $('#pf-loader').value,
              loaderVersion: $('#pf-loaderver').value,
              color: $('#pf-colors .active')?.dataset.color || activeColor,
              ram: Number($('#pf-ram').value) || null,
            };
            try {
              if (isNew) {
                const created = await api.profiles.create(patch);
                await api.profiles.select(created.id);
              } else {
                await api.profiles.update(profile.id, patch);
              }
              await refreshState();
              close();
            } catch (err) {
              toast(err.message, 'err');
            }
          },
        },
      ],
      onOpen: (body) => {
        const preview = $('#pf-preview', body);
        const clearBtn = $('#pf-clear-cover', body);

        $$('#pf-colors .swatch', body).forEach((b) => {
          b.onclick = () => {
            $$('#pf-colors .swatch', body).forEach((x) => x.classList.remove('active'));
            b.classList.add('active');
            // The colour only shows through when there is no picture.
            if (!state.covers.has(data.id)) {
              preview.setAttribute('style', `background:${b.dataset.color}`);
              preview.textContent = ($('#pf-name', body).value || '?').trim().slice(0, 1);
            }
          };
        });

        $('#pf-pick-cover', body).onclick = async () => {
          try {
            const url = await api.profiles.pickCover(profile.id);
            if (!url) return;
            state.covers.set(profile.id, url);
            preview.setAttribute('style', `background-image:url(${url})`);
            preview.textContent = '';
            clearBtn.hidden = false;
            await refreshState();
          } catch (err) { toast(err.message, 'err'); }
        };

        clearBtn.onclick = async () => {
          try {
            await api.profiles.clearCover(profile.id);
            state.covers.delete(profile.id);
            const colour = $('#pf-colors .active', body)?.dataset.color || activeColor;
            preview.setAttribute('style', `background:${colour}`);
            preview.textContent = ($('#pf-name', body).value || '?').trim().slice(0, 1);
            clearBtn.hidden = true;
            await refreshState();
          } catch (err) { toast(err.message, 'err'); }
        };
        const ram = $('#pf-ram', body);
        ram.oninput = () => {
          $('#pf-ram-label', body).textContent = Number(ram.value) ? fmtGB(ram.value) : t('profiles.ramInherit');
        };
        const loadLoaderVersions = async () => {
          const loader = $('#pf-loader', body).value;
          const field = $('#pf-loaderver-field', body);
          const select = $('#pf-loaderver', body);
          field.hidden = loader === 'vanilla';
          if (loader === 'vanilla') return;
          select.innerHTML = `<option value="">${esc(t('profiles.latest'))}</option>`;
          try {
            const list = await api.versions.loaderVersions(loader, $('#pf-version', body).value);
            select.innerHTML += list.slice(0, 60).map((v) =>
              `<option value="${esc(v.version)}"${v.version === data.loaderVersion ? ' selected' : ''}>${esc(v.version)}${v.stable ? '' : ' (beta)'}</option>`).join('');
          } catch {
            select.innerHTML += `<option value="" disabled>${esc(t('profiles.loaderVersionFailed'))}</option>`;
          }
        };
        $('#pf-loader', body).onchange = loadLoaderVersions;
        $('#pf-version', body).onchange = loadLoaderVersions;
        loadLoaderVersions();
      },
    });
  }

  // =========================================================================
  // accounts
  // =========================================================================

  function renderAccounts() {
    const grid = $('#account-grid');
    if (!state.accounts.length) {
      grid.innerHTML = `<div class="empty"><div class="et">${esc(t('accounts.none'))}</div>${esc(t('accounts.noneHint'))}</div>`;
      return;
    }
    grid.innerHTML = state.accounts.map((a) => {
      const selected = a.uuid === state.account?.uuid;
      const skin = state.skins.get(a.uuid);
      return `<div class="card account-card ${selected ? 'selected' : ''}" data-uuid="${esc(a.uuid)}">
        <div class="card-top">
          <img class="mod-icon" src="${skin?.head || ''}" alt="" />
          <div style="min-width:0;flex:1">
            <div class="card-title">${esc(a.name)}</div>
            <div class="card-sub">${a.type === 'offline' ? esc(t('account.offline')) : 'Microsoft'}</div>
          </div>
          ${selected ? `<span class="badge ok">${esc(t('accounts.inUse'))}</span>` : ''}
          ${a.expired ? `<span class="badge warn">${esc(t('accounts.expired'))}</span>` : ''}
        </div>
        <div class="card-actions">
          <button class="btn" data-act="use"${selected ? ' disabled' : ''}>${esc(t('accounts.use'))}</button>
          ${a.type !== 'offline' ? `<button class="btn" data-act="skin" data-icon="download"><span>${esc(t('accounts.changeSkin'))}</span></button>` : ''}
          <button class="btn icon-only danger" data-act="del" data-icon="trash" title="${esc(t('accounts.remove'))}"></button>
        </div>
      </div>`;
    }).join('');
    hydrateIcons(grid);

    $$('.account-card', grid).forEach((card) => {
      const uuid = card.dataset.uuid;
      $$('[data-act]', card).forEach((btn) => {
        btn.onclick = async () => {
          try {
            switch (btn.dataset.act) {
              case 'use':
                await api.auth.select(uuid);
                await refreshState();
                break;
              case 'skin': {
                const skin = await api.skins.upload(uuid, 'classic');
                if (skin) { await applySkin(uuid, skin); toast(t('toast.saved'), 'ok'); }
                break;
              }
              case 'del':
                await api.auth.remove(uuid);
                await refreshState();
                break;
            }
          } catch (err) {
            toast(err.message, 'err');
          }
        };
      });
    });
  }

  $('#btn-add-msa').onclick = async () => {
    const btn = $('#btn-add-msa');
    btn.disabled = true;
    try {
      const account = await api.auth.signIn();
      if (!account) return; // window closed without finishing
      await refreshState();
      loadSkin(account.uuid);
      toast(t('accounts.signedIn', { name: account.name }), 'ok');
    } catch (err) {
      toast(err.message, 'err');
    } finally {
      btn.disabled = false;
    }
  };

  $('#btn-add-offline').onclick = () => {
    openModal({
      title: t('accounts.offlineTitle'),
      body: `
        <div class="field">
          <label>${esc(t('accounts.offlineName'))}</label>
          <input id="off-name" maxlength="16" placeholder="Steve" />
          <div class="hint">${esc(t('accounts.offlineHint'))}</div>
        </div>`,
      buttons: [
        { label: t('btn.cancel'), onClick: (close) => close() },
        { label: t('btn.add'), brand: true, onClick: async (close) => {
          try {
            const account = await api.auth.addOffline($('#off-name').value);
            close();
            await refreshState();
            loadSkin(account.uuid);
          } catch (err) {
            toast(err.message, 'err');
          }
        } },
      ],
      onOpen: (body) => {
        const input = $('#off-name', body);
        input.focus();
        input.onkeydown = (e) => { if (e.key === 'Enter') $('#modal-foot .brand').click(); };
      },
    });
  };

  // =========================================================================
  // mods
  // =========================================================================

  const QUICK_PICKS = ['sodium', 'lithium', 'iris', 'fabric-api'];
  const PERF_PACK = ['sodium', 'lithium', 'ferrite-core', 'entityculling', 'modernfix', 'immediatelyfast'];

  $$('#mod-tabs .tab').forEach((btn) => {
    btn.onclick = () => {
      $$('#mod-tabs .tab').forEach((b) => b.classList.toggle('active', b === btn));
      $('#mods-browse').hidden = btn.dataset.tab !== 'browse';
      $('#mods-installed').hidden = btn.dataset.tab !== 'installed';
      if (btn.dataset.tab === 'installed') renderInstalledMods();
      else renderMods();
    };
  });

  let searchTimer = null;
  $('#mod-search').oninput = () => {
    clearTimeout(searchTimer);
    searchTimer = setTimeout(() => { state.modSearch.offset = 0; renderMods(); }, 320);
  };
  $('#mod-type').onchange = () => { state.modSearch.offset = 0; renderMods(); };
  $('#mod-sort').onchange = () => { state.modSearch.offset = 0; renderMods(); };
  $('#mod-filter-version').onchange = () => { state.modSearch.offset = 0; renderMods(); };
  $('#btn-load-more').onclick = () => { state.modSearch.offset += 30; renderMods(true); };
  $('#btn-open-mods-folder').onclick = () => state.profile && api.app.openFolder(state.profile.id);

  function renderQuickPicks() {
    const wrap = $('#quickpicks');
    wrap.innerHTML = `<span class="ql">${esc(t('mods.quickLabel'))}</span>`
      + `<button class="btn sm" data-perf="1">${esc(t('mods.perfPack'))}</button>`
      + QUICK_PICKS.map((slug) => `<button class="btn sm" data-slug="${slug}">${slug}</button>`).join('');

    $('[data-perf]', wrap).onclick = async (e) => {
      if (!state.profile) return;
      const btn = e.currentTarget;
      btn.disabled = true;
      let installed = 0;
      for (const slug of PERF_PACK) {
        try { await api.modrinth.install(state.profile.id, slug); installed++; } catch { /* not available for this version */ }
      }
      btn.disabled = false;
      await refreshState();
      renderMods();
      toast(installed ? t('mods.perfInstalled', { n: installed }) : t('mods.noResults'), installed ? 'ok' : 'err');
    };
    $$('[data-slug]', wrap).forEach((b) => { b.onclick = () => installMod(b.dataset.slug, b); });
  }

  async function renderMods(append = false) {
    if (!state.profile) {
      $('#mod-grid').innerHTML = `<div class="empty"><div class="et">${esc(t('profiles.none'))}</div>${esc(t('mods.needProfile'))}</div>`;
      return;
    }
    renderQuickPicks();
    $('#mod-filter-label').textContent = state.profile.mcVersion;
    $('#mods-sub').textContent = t('mods.sub', {
      profile: state.profile.name,
      version: state.profile.mcVersion,
      loader: LOADER_LABEL[state.profile.loader] || state.profile.loader,
    });

    const grid = $('#mod-grid');
    if (!append) grid.innerHTML = `<div class="empty"><span class="loading-row"><span class="spinner"></span>${esc(t('mods.searching'))}</span></div>`;

    try {
      const res = await api.modrinth.search({
        query: $('#mod-search').value.trim(),
        loader: state.profile.loader,
        gameVersion: $('#mod-filter-version').checked ? state.profile.mcVersion : '',
        projectType: $('#mod-type').value,
        index: $('#mod-sort').value,
        limit: 30,
        offset: state.modSearch.offset,
      });

      const installedIds = new Set((state.profile.mods || []).map((m) => m.projectId));
      const cards = res.hits.map((hit) => {
        const isInstalled = installedIds.has(hit.project_id);
        return `<div class="card mod-card" data-id="${esc(hit.project_id)}" data-slug="${esc(hit.slug)}" data-type="${esc(hit.project_type)}">
          ${hit.icon_url
            ? `<img class="mod-icon" src="${esc(hit.icon_url)}" alt="" loading="lazy" />`
            : `<span class="mod-icon" data-icon="package"></span>`}
          <div class="mod-body">
            <div class="card-title">${esc(hit.title)}</div>
            <div class="card-sub">${esc(hit.author)}</div>
            <div class="mod-desc">${esc(hit.description)}</div>
            <div class="mod-foot">
              <div class="mod-stats">
                <span>${icon('download')}${fmtNumber(hit.downloads)}</span>
                <span>${icon('heart')}${fmtNumber(hit.follows)}</span>
              </div>
              <button class="btn sm${isInstalled ? '' : ' brand'}" data-act="install"${isInstalled ? ' disabled' : ''}>
                ${esc(isInstalled ? t('mods.installedBadge') : (hit.project_type === 'modpack' ? t('mods.installPack') : t('mods.install')))}
              </button>
            </div>
          </div>
        </div>`;
      }).join('');

      if (append) grid.insertAdjacentHTML('beforeend', cards);
      else grid.innerHTML = cards || `<div class="empty"><div class="et">${esc(t('mods.noResults'))}</div>${esc(t('mods.noResultsHint'))}</div>`;
      hydrateIcons(grid);

      $('#btn-load-more').hidden = res.hits.length < 30;

      $$('.mod-card [data-act="install"]', grid).forEach((btn) => {
        if (btn.dataset.bound) return;
        btn.dataset.bound = '1';
        btn.onclick = (e) => {
          e.stopPropagation();
          const card = btn.closest('.mod-card');
          if (card.dataset.type === 'modpack') installModpack(card.dataset.slug, btn);
          else installMod(card.dataset.slug || card.dataset.id, btn);
        };
      });
      $$('.mod-card', grid).forEach((card) => {
        card.onclick = (e) => {
          if (e.target.closest('button')) return;
          api.openExternal(`https://modrinth.com/${card.dataset.type}/${card.dataset.slug}`);
        };
      });
    } catch (err) {
      grid.innerHTML = `<div class="empty"><div class="et">${esc(t('mods.searchFailed'))}</div>${esc(err.message)}</div>`;
    }
  }

  async function installMod(slug, btn) {
    if (!state.profile) return;
    const original = btn?.textContent;
    if (btn) { btn.disabled = true; btn.textContent = t('mods.installing'); }
    try {
      const installed = await api.modrinth.install(state.profile.id, slug);
      await refreshState();
      toast(t('mods.installedOk', { name: installed[0]?.title || slug }), 'ok');
      if (btn) { btn.textContent = t('mods.installedBadge'); btn.classList.remove('brand'); }
    } catch (err) {
      toast(err.message, 'err');
      if (btn) { btn.textContent = original; btn.disabled = false; }
    }
  }

  async function installModpack(slug, btn) {
    if (!state.profile) return;
    const original = btn?.textContent;
    if (btn) { btn.disabled = true; btn.textContent = t('mods.installing'); }
    try {
      const versions = await api.modrinth.versions(slug, {});
      if (!versions.length) throw new Error(t('mods.noResults'));
      const info = await api.modrinth.installModpack(state.profile.id, versions[0].id);
      await refreshState();
      toast(t('mods.packInstalled', { name: info.name, n: info.fileCount }), 'ok');
    } catch (err) {
      toast(err.message, 'err');
    } finally {
      if (btn) { btn.disabled = false; btn.textContent = original; }
    }
  }

  /**
   * The installed-mods list, shared by the Start view and the Mods view.
   * Each row carries an enable/disable switch and a delete button; deleting
   * always asks first, since it removes the jar from disk.
   */
  function renderModList(list) {
    if (!list) return;
    const mods = state.profile?.mods || [];
    if (!mods.length) {
      list.innerHTML = `<div class="empty"><div class="et">${esc(t('mods.none'))}</div>${esc(t('mods.noneHint'))}</div>`;
      return;
    }
    list.innerHTML = mods.map((m) => `
      <div class="row ${m.enabled === false ? 'off' : ''}" data-id="${esc(m.projectId)}">
        ${m.icon ? `<img class="mod-icon" src="${esc(m.icon)}" alt="" />` : `<span class="mod-icon" data-icon="package"></span>`}
        <div class="row-meta">
          <div class="row-name">${esc(m.title)}
            ${m.dependency ? `<span class="badge">${esc(t('mods.dependency'))}</span>` : ''}
            ${m.update ? `<span class="badge warn">${esc(t('mods.updateAvailable'))}</span>` : ''}
          </div>
          <div class="row-sub">${esc(m.versionNumber)} · ${esc(m.filename)}</div>
        </div>
        <span class="row-state">${esc(m.enabled === false ? t('mods.off') : t('mods.on'))}</span>
        <label class="switch" data-tip-text="${esc(t('mods.toggle'))}">
          <input type="checkbox" data-act="toggle" ${m.enabled === false ? '' : 'checked'} /><span class="slider"></span>
        </label>
        ${m.update ? `<button class="btn sm brand" data-act="update">${esc(t('mods.update'))}</button>` : ''}
        <button class="btn sm icon-only danger" data-act="remove" data-icon="trash"
                data-tip-text="${esc(t('mods.remove'))}"></button>
      </div>`).join('');
    hydrateIcons(list);

    $$('.row', list).forEach((row) => {
      const id = row.dataset.id;
      const mod = (state.profile.mods || []).find((m) => m.projectId === id);

      $('[data-act="toggle"]', row).onchange = async (e) => {
        const enabled = e.target.checked;
        try {
          await api.modrinth.toggle(state.profile.id, id, enabled);
          await refreshState();
        } catch (err) {
          e.target.checked = !enabled; // put the switch back where it was
          toast(err.message, 'err');
        }
      };

      $('[data-act="remove"]', row).onclick = () => {
        confirmDialog({
          title: t('mods.deleteTitle'),
          text: t('mods.deleteConfirm', { name: mod?.title || id }),
          confirmLabel: t('btn.delete'),
          onConfirm: async () => {
            try {
              await api.modrinth.uninstall(state.profile.id, id);
              await refreshState();
              toast(t('mods.deleted', { name: mod?.title || id }), 'ok');
            } catch (err) { toast(err.message, 'err'); }
          },
        });
      };

      const upd = $('[data-act="update"]', row);
      if (upd) {
        upd.onclick = async () => {
          await installMod(id, upd);
          if (mod) delete mod.update;
          renderModList(list);
        };
      }
    });
  }

  function renderInstalledMods() {
    renderModList($('#installed-list'));
  }

  $('#btn-check-updates').onclick = async () => {
    if (!state.profile) return;
    const btn = $('#btn-check-updates');
    btn.disabled = true;
    try {
      const updates = await api.modrinth.checkUpdates(state.profile.id);
      for (const mod of state.profile.mods || []) {
        const hit = updates.find((u) => u.projectId === mod.projectId);
        if (hit) mod.update = hit; else delete mod.update;
      }
      renderInstalledMods();
      toast(updates.length ? t('mods.updatesFound', { n: updates.length }) : t('mods.upToDate'),
        updates.length ? 'info' : 'ok');
    } catch (err) {
      toast(err.message, 'err');
    } finally {
      btn.disabled = false;
    }
  };

  // =========================================================================
  // servers
  // =========================================================================

  /** Servers send their own icon in the status response; fall back to a tile. */
  function serverIconHtml(server, size = 'sm') {
    const favicon = server.status?.favicon;
    return favicon
      ? `<img class="cover ${size} server-icon" src="${esc(favicon)}" alt="" />`
      : `<span class="cover ${size}" style="background:${esc(tileColor(server.id || server.address))}">${esc(server.name.slice(0, 1))}</span>`;
  }

  function renderServerList() {
    const list = $('#server-list');
    list.innerHTML = state.servers.map((s) => {
      const st = s.status;
      const players = st?.online
        ? `<span class="sr-players">${st.players.online}/${st.players.max}</span>`
        : st ? `<span class="sr-players off">${esc(t('servers.offline'))}</span>`
        : '<span class="sr-players"><span class="spinner"></span></span>';
      return `<button class="server-row" data-addr="${esc(s.address)}">
        ${serverIconHtml(s)}
        <span class="sr-meta">
          <span class="sr-name"><span class="dot ${!st ? '' : st.online ? 'up' : 'down'}"></span>${esc(s.name)}</span>
          <span class="sr-addr">${esc(s.address)}</span>
        </span>
        ${players}
      </button>`;
    }).join('');
    $$('.server-row', list).forEach((row) => { row.onclick = () => joinServer(row.dataset.addr); });
  }

  function renderServerGrid() {
    const grid = $('#server-grid');
    grid.innerHTML = state.servers.map((s) => {
      const st = s.status;
      return `<div class="card server-card">
        <div class="card-top">
          ${serverIconHtml(s, '')}
          <div style="flex:1;min-width:0">
            <div class="card-title"><span class="dot ${!st ? '' : st.online ? 'up' : 'down'}"></span> ${esc(s.name)}</div>
            <div class="card-sub">${esc(s.address)}${s.tag ? ` · ${esc(s.tag)}` : ''}</div>
          </div>
        </div>
        <div class="sc-motd">${esc(st?.motd || (st ? t('servers.offline') : t('servers.pinging')))}</div>
        ${st?.online ? `<div class="sc-stats">
          <span>${st.players.online}/${st.players.max} ${esc(t('servers.players'))}</span>
          <span>${st.ping} ms</span>
          <span class="sc-version">${esc(st.version)}</span>
        </div>` : ''}
        <button class="btn sc-join" data-addr="${esc(s.address)}"${st && !st.online ? ' disabled' : ''}>${esc(t('servers.join'))}</button>
      </div>`;
    }).join('');
    hydrateIcons(grid);

    $$('[data-addr]', grid).forEach((b) => { b.onclick = () => joinServer(b.dataset.addr); });
  }

  async function joinServer(address) {
    if (!state.profile) { showView('profiles'); return; }
    if (!state.account) { toast(t('launch.needAccount'), 'err'); showView('accounts'); return; }
    showView('start');
    await doLaunch(state.profile.id, address);
  }

  $('#btn-refresh-servers').onclick = () => pingServers();
  async function pingServers() {
    const list = await api.servers.partners();
    state.servers = list.map((s) => ({ ...s, status: null }));
    renderServerList();
    if ($('#view-servers').classList.contains('active')) renderServerGrid();
    api.servers.pingAll(state.servers.map(({ status, ...rest }) => rest));
  }

  api.on.serverPinged((result) => {
    const idx = state.servers.findIndex((s) => s.address === result.address);
    if (idx < 0) return;
    state.servers[idx] = { ...state.servers[idx], status: result.status };
    // Busiest first, so the side list leads with somewhere worth joining.
    state.servers.sort((a, b) => (b.status?.players?.online ?? -1) - (a.status?.players?.online ?? -1));
    renderServerList();
    if ($('#view-servers').classList.contains('active')) renderServerGrid();
  });

  // =========================================================================
  // settings
  // =========================================================================

  function renderSettings() {
    const c = state.config;
    $('#settings-grid').innerHTML = `
      <div class="settings-card">
        <h2><span data-icon="monitor"></span>${esc(t('settings.game'))}</h2>
        <div class="sc-sub">${esc(t('settings.gameSub'))}</div>
        <div class="field">
          <div class="range-head"><label>${esc(t('settings.ram'))}</label><b id="set-ram-label">${esc(fmtGB(c.ram))}</b></div>
          <input type="range" id="set-ram" min="1024" max="${state.totalRam}" step="512" value="${c.ram}" />
          <div class="hint">${esc(t('settings.ramHint', { total: fmtGB(state.totalRam) }))}</div>
        </div>
        <div class="field">
          <label>${esc(t('settings.resolution'))}</label>
          <div class="field-row">
            <input type="number" id="set-w" value="${c.width}" min="640" />
            <span style="color:var(--text-3)">×</span>
            <input type="number" id="set-h" value="${c.height}" min="480" />
          </div>
        </div>
        <div class="toggle-row">
          <div><div class="toggle-label">${esc(t('settings.fullscreen'))}</div><div class="toggle-desc">${esc(t('settings.fullscreenDesc'))}</div></div>
          <label class="switch"><input type="checkbox" id="set-fs" ${c.fullscreen ? 'checked' : ''} /><span class="slider"></span></label>
        </div>
      </div>

      <div class="settings-card">
        <h2><span data-icon="gauge"></span>${esc(t('settings.performance'))}</h2>
        <div class="sc-sub">${esc(t('settings.performanceSub'))}</div>
        <div class="preset-row" id="set-presets">
          ${['potato', 'balanced', 'quality'].map((key) => `
            <button class="preset ${c.performancePreset === key ? 'active' : ''}" data-preset="${key}">
              <span class="pn">${esc(t(`settings.preset.${key}`))}</span>
              <span class="pd">${esc(t(`settings.preset.${key}D`))}</span>
            </button>`).join('')}
        </div>
        <div class="field" style="margin-top:16px">
          <label>${esc(t('settings.downloads'))}</label>
          <input type="number" id="set-dl" value="${c.concurrentDownloads}" min="1" max="32" />
          <div class="hint">${esc(t('settings.downloadsHint'))}</div>
        </div>
      </div>

      <div class="settings-card">
        <h2><span data-icon="coffee"></span>${esc(t('settings.java'))}</h2>
        <div class="sc-sub">${esc(t('settings.javaSub'))}</div>
        <div class="field">
          <label>${esc(t('settings.javaPath'))}</label>
          <div class="field-row">
            <input id="set-java" value="${esc(c.javaPath)}" placeholder="${esc(t('settings.javaAuto'))}" />
            <button class="btn" id="btn-java-browse">${esc(t('settings.javaBrowse'))}</button>
          </div>
          <div class="hint" id="java-detected">…</div>
        </div>
        <div class="field">
          <label>${esc(t('settings.javaArgs'))}</label>
          <input id="set-jargs" value="${esc(c.javaArgs)}" placeholder="-XX:+UseG1GC" />
        </div>
      </div>

      <div class="settings-card">
        <h2><span data-icon="window"></span>${esc(t('settings.launcher'))}</h2>
        <div class="sc-sub">${esc(t('settings.launcherSub'))}</div>
        <div class="field">
          <label>${esc(t('settings.language'))}</label>
          <select id="set-lang">
            <option value="de"${c.language === 'de' ? ' selected' : ''}>Deutsch</option>
            <option value="en"${c.language === 'en' ? ' selected' : ''}>English</option>
          </select>
        </div>
        <div class="toggle-row">
          <div><div class="toggle-label">${esc(t('settings.keepOpen'))}</div><div class="toggle-desc">${esc(t('settings.keepOpenDesc'))}</div></div>
          <label class="switch"><input type="checkbox" id="set-keep" ${c.keepLauncherOpen ? 'checked' : ''} /><span class="slider"></span></label>
        </div>
        <div class="toggle-row">
          <div><div class="toggle-label">${esc(t('settings.hideOnLaunch'))}</div><div class="toggle-desc">${esc(t('settings.hideOnLaunchDesc'))}</div></div>
          <label class="switch"><input type="checkbox" id="set-hide" ${c.hideOnLaunch ? 'checked' : ''} /><span class="slider"></span></label>
        </div>
        <div class="toggle-row">
          <div><div class="toggle-label">${esc(t('settings.snapshots'))}</div><div class="toggle-desc">${esc(t('settings.snapshotsDesc'))}</div></div>
          <label class="switch"><input type="checkbox" id="set-snap" ${c.showSnapshots ? 'checked' : ''} /><span class="slider"></span></label>
        </div>
      </div>

      <div class="settings-card">
        <h2><span data-icon="database"></span>${esc(t('settings.storage'))}</h2>
        <div class="sc-sub">${esc(t('settings.storageSub'))}</div>
        <div class="field"><input readonly value="${esc(state.paths?.root || '')}" /></div>
        <div class="field-row">
          <button class="btn" id="btn-open-root" data-icon="folder"><span>${esc(t('settings.openRoot'))}</span></button>
          <button class="btn" id="btn-open-logs" data-icon="folder"><span>${esc(t('settings.openLogs'))}</span></button>
        </div>
      </div>

      <div class="settings-card">
        <h2><span data-icon="download"></span>${esc(t('settings.updates'))}</h2>
        <div class="sc-sub">${esc(t('settings.updatesSub'))}</div>
        <div class="field">
          <label>${esc(t('update.installedVersion'))}</label>
          <input readonly value="MangoClient ${esc(state.version || '')}" />
          <div class="hint" id="update-status">…</div>
        </div>
        <div class="field-row">
          <button class="btn" id="btn-update-check" data-icon="refresh"><span>${esc(t('update.check'))}</span></button>
          <button class="btn brand" id="btn-update-install" hidden>${esc(t('update.install'))}</button>
        </div>
      </div>`;
    hydrateIcons($('#settings-grid'));

    const save = (patch) => api.app.setConfig(patch).then((cfg) => { state.config = cfg; });

    const ram = $('#set-ram');
    ram.oninput = () => { $('#set-ram-label').textContent = fmtGB(ram.value); };
    ram.onchange = () => save({ ram: Number(ram.value) }).then(renderStart);

    $('#set-w').onchange = (e) => save({ width: Number(e.target.value) });
    $('#set-h').onchange = (e) => save({ height: Number(e.target.value) });
    $('#set-fs').onchange = (e) => save({ fullscreen: e.target.checked });
    $('#set-dl').onchange = (e) => save({ concurrentDownloads: Number(e.target.value) });
    $('#set-java').onchange = (e) => save({ javaPath: e.target.value.trim() });
    $('#set-jargs').onchange = (e) => save({ javaArgs: e.target.value });
    $('#set-keep').onchange = (e) => save({ keepLauncherOpen: e.target.checked });
    $('#set-hide').onchange = (e) => save({ hideOnLaunch: e.target.checked });
    $('#set-snap').onchange = (e) => save({ showSnapshots: e.target.checked });
    $('#set-lang').onchange = async (e) => {
      window.i18n.setLanguage(e.target.value);
      await save({ language: e.target.value });
      renderAll();
      renderSettings();
    };

    $$('#set-presets .preset').forEach((b) => {
      b.onclick = () => {
        $$('#set-presets .preset').forEach((x) => x.classList.remove('active'));
        b.classList.add('active');
        save({ performancePreset: b.dataset.preset });
      };
    });

    $('#btn-java-browse').onclick = async () => {
      try {
        const info = await api.java.pick();
        if (info) { $('#set-java').value = info.path; await save({ javaPath: info.path }); }
      } catch (err) { toast(err.message, 'err'); }
    };
    $('#btn-open-root').onclick = () => api.app.openFolder('root');
    $('#btn-open-logs').onclick = () => api.app.openFolder('logs');

    $('#btn-update-check').onclick = async () => {
      try {
        state.update = await api.update.check();
      } catch (err) {
        state.update = { state: 'error', error: err.message };
      }
      renderUpdate();
    };
    $('#btn-update-install').onclick = () => api.update.install().catch((err) => toast(err.message, 'err'));
    renderUpdateCard();

    api.java.list().then(({ system, managed }) => {
      const all = [...managed, ...system];
      $('#java-detected').textContent = all.length
        ? t('settings.javaDetected', { list: all.map((j) => `Java ${j.major}`).join(', ') })
        : t('settings.javaNone');
    }).catch(() => {});
  }

  // =========================================================================
  // console
  // =========================================================================

  function pushConsole(line, level = 'info') {
    const cls = level === 'error' ? 'err' : /WARN/i.test(line) ? 'warn' : /ERROR|Exception/i.test(line) ? 'err' : '';
    state.consoleLines.push({ line, cls });
    if (state.consoleLines.length > 3000) state.consoleLines.splice(0, 500);

    const out = $('#console-out');
    const span = document.createElement('span');
    span.className = cls;
    span.textContent = `${line}\n`;
    out.appendChild(span);
    while (out.childNodes.length > 3000) out.removeChild(out.firstChild);
    scrollConsole();
  }

  function scrollConsole() {
    const out = $('#console-out');
    out.scrollTop = out.scrollHeight;
  }

  // =========================================================================
  // events from main
  // =========================================================================

  api.on.launchLog(({ line, level }) => pushConsole(line, level));

  api.on.launchProgress(({ phase, done, total, label }) => {
    const weights = { java: [0, 15], client: [15, 22], libraries: [22, 55], assets: [55, 98], modpack: [10, 95], done: [100, 100] };
    const [from, to] = weights[phase] || [0, 100];
    const ratio = total ? done / total : 1;
    const phaseLabel = {
      java: t('launch.java'), client: t('launch.downloading'),
      libraries: t('launch.libraries'), assets: t('launch.assets'),
      modpack: t('launch.modpack'), done: t('launch.launching'),
    }[phase] || label;
    setProgress(from + (to - from) * ratio, total > 1 ? `${phaseLabel} (${done}/${total})` : phaseLabel);
  });

  api.on.launchState(({ profileId, state: s }) => {
    state.launchState = s;
    if (s === 'running') {
      state.running.add(profileId);
      $('#launch-progress').hidden = true;
      viewer?.setWalking(true);
    } else if (s === 'stopped' || s === 'crashed') {
      state.running.delete(profileId);
      $('#launch-progress').hidden = true;
      viewer?.setWalking(false);
      if (s === 'crashed') {
        toast(t('launch.crashed'), 'err');
        $('#console-drawer').hidden = false;
      }
      refreshState();
    }
    updatePlayButton();
    renderStart();
    if ($('#view-profiles').classList.contains('active')) renderProfiles();
  });

  api.on.skinUpdated(async (payload) => { await applySkin(payload.uuid, payload); });

  // =========================================================================
  // bootstrap
  // =========================================================================

  async function refreshState() {
    const s = await api.app.state();
    state.config = s.config;
    state.profiles = s.profiles;
    state.accounts = s.accounts;
    state.profile = s.selectedProfile;
    state.account = s.selectedAccount;
    state.totalRam = s.totalRam;
    state.paths = s.paths;
    state.running = new Set(s.running || []);
    state.version = s.version;
    state.covers = new Map(Object.entries(await api.profiles.covers().catch(() => ({}))));
    renderAll();
  }

  function renderAll() {
    window.i18n.applyStatic();
    renderStart();
    if ($('#view-profiles').classList.contains('active')) renderProfiles();
    if ($('#view-accounts').classList.contains('active')) renderAccounts();
    if ($('#view-settings').classList.contains('active')) renderSettings();
    if ($('#view-servers').classList.contains('active')) renderServerGrid();
    if ($('#view-mods').classList.contains('active') && !$('#mods-installed').hidden) renderInstalledMods();
    renderServerList();
  }

  async function init() {
    $('#brand-mark').innerHTML = logoMark(24);
    $('#uo-mark').innerHTML = logoMark(56);
    hydrateIcons();
    await refreshState();
    window.i18n.setLanguage(state.config.language || 'de');
    document.body.classList.toggle('sidebar-off', state.config.sidebarOpen === false);
    state.update = await api.update.state().catch(() => null);
    renderUpdate();
    renderView('start');
    ensureViewer();

    await loadDefaultSkin();
    if (state.account) loadSkin(state.account.uuid);
    for (const a of state.accounts) if (a.uuid !== state.account?.uuid) loadSkin(a.uuid);

    pingServers();

    api.versions.manifest().then((m) => { state.versions = m; }).catch((err) => {
      console.warn('version manifest failed', err);
      state.versions = { versions: [{ id: state.profile?.mcVersion || '1.21.11', type: 'release' }], latest: {}, installed: [] };
    });

    renderAll();
  }

  init().catch((err) => {
    console.error(err);
    document.body.innerHTML = `<div style="padding:40px;color:#e08a82;font-family:ui-monospace,monospace">Startup failed: ${esc(err.message)}</div>`;
  });
})();
