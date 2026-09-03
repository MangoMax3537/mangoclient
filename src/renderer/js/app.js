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
    modUpdates: new Map(),
    consoleLines: [],
    instanceTab: 'overview',
    instanceContentType: 'mod',
    shots: [],
    shotIndex: 0,
    logs: [],
    statsDays: 14,
    settingsSection: 'game',
  };

  let viewer = null;
  let viewerSkin = null;
  let viewerUnavailable = false;
  let homeViewer = null;
  let homeViewerSkin = null;
  let homeViewerUnavailable = false;
  let versionsPromise = null;
  const loadingSkins = new Set();
  const loadingCovers = new Set();
  const CONSOLE_LIMIT = 800;
  const CONSOLE_TRIM = 200;
  const CONSOLE_BYTE_LIMIT = 512 * 1024;
  let consoleBytes = 0;

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

  function fmtChartDuration(ms) {
    if (!ms) return '0 min';
    if (ms < 60000) return '<1 min';
    return fmtDuration(ms);
  }

  function fmtDate(ts) {
    if (!ts) return t('profiles.never');
    return new Date(ts).toLocaleDateString(window.i18n.lang === 'de' ? 'de-DE' : 'en-GB',
      { day: '2-digit', month: '2-digit', year: '2-digit' });
  }

  function fmtGB(mb) {
    return `${(mb / 1024).toFixed(1)} GB`;
  }

  function fmtBytes(bytes) {
    const units = ['B', 'KB', 'MB', 'GB', 'TB'];
    let value = Number(bytes) || 0;
    let unit = 0;
    while (value >= 1024 && unit < units.length - 1) { value /= 1024; unit++; }
    return `${value < 10 && unit > 0 ? value.toFixed(1) : Math.round(value)} ${units[unit]}`;
  }

  function fmtDateTime(ts) {
    if (!ts) return t('profiles.never');
    return new Date(ts).toLocaleString(window.i18n.lang === 'de' ? 'de-DE' : 'en-GB',
      { day: '2-digit', month: '2-digit', year: '2-digit', hour: '2-digit', minute: '2-digit' });
  }

  const LOADER_LABEL = { vanilla: 'Vanilla', fabric: 'Fabric', quilt: 'Quilt', neoforge: 'NeoForge', forge: 'Forge' };
  const CONTENT_TYPES = ['mod', 'resourcepack', 'shader'];
  const CONTENT_ICONS = { mod: 'package', resourcepack: 'image', shader: 'layers' };

  /** Old profiles have no type on mod records, so an absent value still means mod. */
  function contentType(record) {
    return record?.type || 'mod';
  }

  function profileContent(profile, type = null) {
    const entries = profile?.mods || [];
    return type ? entries.filter((entry) => contentType(entry) === type) : entries;
  }

  function dependencyOwners(profile, projectId) {
    return (profile?.mods || []).filter((candidate) =>
      candidate.projectId !== projectId
      && (candidate.requiredDependencies || []).some((dependency) => dependency.projectId === projectId));
  }

  function contentLabel(type) {
    if (type === 'resourcepack') return t('mods.type.rp');
    if (type === 'shader') return t('mods.type.shader');
    return t('mods.type.mod');
  }

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

  /** The facts under an instance's name, shared by Start and the instance page. */
  function factsHtml(profile, extra = []) {
    const ram = profile.ram || state.config.ram || 4096;
    const loader = LOADER_LABEL[profile.loader] || profile.loader;
    return [
      [t('facts.version'), profile.mcVersion],
      [t('facts.loader'), profile.loaderVersion ? `${loader} ${profile.loaderVersion}` : loader],
      [t('facts.java'), javaBadgeFor(profile.mcVersion)],
      [t('facts.ram'), fmtGB(ram)],
      [t('facts.mods'), String(profileContent(profile, 'mod').length)],
      [t('facts.lastPlayed'), fmtDate(profile.lastPlayed)],
      ...extra,
    ].map(([k, v]) => `<div class="fact"><span class="k">${esc(k)}</span><span class="v">${esc(v)}</span></div>`)
      .join('');
  }

  /** Home keeps the selected profile readable at a glance instead of showing
   * every runtime detail with equal weight. Version/loader/Java sit by the
   * title; these four values answer the questions players check before Play. */
  function homeFactsHtml(profile) {
    const ram = profile.ram || state.config.ram || 4096;
    return [
      ['package', String(profileContent(profile, 'mod').length), t('facts.mods')],
      ['clock', fmtDuration(profile.playTimeMs || 0), t('instance.playTime')],
      ['gauge', fmtGB(ram), t('facts.ram')],
      ['calendar', fmtDate(profile.lastPlayed), t('facts.lastPlayed')],
    ].map(([glyph, value, label]) => `
      <div class="home-stat">
        ${icon(glyph)}
        <div class="home-stat-copy">
          <span class="home-stat-value">${esc(value)}</span>
          <span class="home-stat-label">${esc(label)}</span>
        </div>
      </div>`).join('');
  }

  function coverHtml(profile, size = '') {
    return `<span class="cover ${size}" style="${coverStyle(profile)}">${coverLetter(profile)}</span>`;
  }

  async function ensureCovers(ids, { rerender = true } = {}) {
    const wanted = ids
      .map((id) => state.profiles.find((p) => p.id === id))
      .filter((p) => p?.cover && !state.covers.has(p.id) && !loadingCovers.has(p.id));
    if (!wanted.length) return;

    wanted.forEach((p) => loadingCovers.add(p.id));
    try {
      const entries = await Promise.all(wanted.map(async (p) => [p.id, await api.profiles.cover(p.id).catch(() => null)]));
      for (const [id, url] of entries) {
        if (url) state.covers.set(id, url);
      }
    } finally {
      wanted.forEach((p) => loadingCovers.delete(p.id));
    }
    if (rerender) renderAll();
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
    else closeHomeProfileMenu();
  });

  // =========================================================================
  // navigation
  // =========================================================================

  /** Browser-style history so the statusbar's arrows do something real. */
  const history = { stack: ['start'], idx: 0 };

  function renderView(name) {
    document.body.dataset.activeView = name;
    $$('.view').forEach((v) => v.classList.toggle('active', v.id === `view-${name}`));
    if (name !== 'instance') {
      $$('#view-instance .tabpanel').forEach((panel) => panel.replaceChildren());
      state.shots = [];
      state.logs = [];
      state.logId = null;
      if (!$('#lightbox').hidden) closeLightbox();
    }
    $$('.rail-btn').forEach((n) => n.classList.toggle('active', n.dataset.view === name));
    renderRailInstances();
    // An instance page is about one instance, so it says which one.
    $('#breadcrumb').textContent = name === 'instance'
      ? (state.profile?.name || t('nav.instance'))
      : t(`nav.${name}`);
    $('#nav-back').disabled = history.idx === 0;
    $('#nav-fwd').disabled = history.idx >= history.stack.length - 1;

    if (name === 'mods') { renderMods(); syncLocalMods(); }
    if (name === 'servers') renderServerGrid();
    if (name === 'settings') renderSettings();
    if (name === 'profiles') { renderProfiles(); ensureCovers(state.profiles.map((p) => p.id)); }
    if (name === 'accounts') renderAccounts();
    if (name === 'instance') renderInstance();
    if (name === 'stats') renderStats();
    syncViewer();
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
      case 'error':
        if (u.code === 'no-release') return t('update.errorNoRelease');
        if (u.code === 'offline') return t('update.errorOffline');
        return t('update.error', { error: u.error });
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

  function closeHomeProfileMenu() {
    const menu = $('#home-profile-menu');
    const trigger = $('#btn-home-profile');
    if (menu) menu.hidden = true;
    if (trigger) trigger.setAttribute('aria-expanded', 'false');
  }

  document.addEventListener('click', () => {
    $('#account-menu').hidden = true;
    closeHomeProfileMenu();
  });

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
      $('#lp-sub').textContent = `${profile.mcVersion} · ${LOADER_LABEL[profile.loader] || profile.loader} · ${javaBadgeFor(profile.mcVersion)}`;

      const ram = profile.ram || state.config.ram || 4096;
      $('#lp-facts').innerHTML = homeFactsHtml(profile);
      $('#ram-warn').hidden = ram >= 3072;
    } else {
      $('#lp-name').textContent = t('profiles.none');
      $('#lp-sub').textContent = '';
      $('#lp-facts').innerHTML = '';
      $('#ram-warn').hidden = true;
    }
    $('#home-character-label').textContent = state.account?.name || t('account.none');

    updatePlayButton();
    renderAccountChip();
    renderRunPill();
    renderMangoConfigPill();
    renderRailInstances();
    renderModList($('#start-mod-list'), 'mod');
    $('#start-mod-count').textContent = profileContent(state.profile, 'mod').length || '';
  }

  /** Rail quick-switcher: most recently played instance sits at the top. */
  function hideRailInstanceTooltip() {
    const tooltip = $('#rail-instance-tooltip');
    if (tooltip) tooltip.hidden = true;
  }

  function showRailInstanceTooltip(button) {
    const tooltip = $('#rail-instance-tooltip');
    if (!tooltip) return;
    const rect = button.getBoundingClientRect();
    tooltip.textContent = button.getAttribute('aria-label') || '';
    tooltip.hidden = false;
    const top = Math.max(8, Math.min(
      window.innerHeight - tooltip.offsetHeight - 8,
      rect.top + (rect.height - tooltip.offsetHeight) / 2,
    ));
    tooltip.style.top = `${Math.round(top)}px`;
  }

  function renderRailInstances() {
    const rail = $('#rail-instances');
    hideRailInstanceTooltip();
    const ordered = [...state.profiles].sort((a, b) => (b.lastPlayed || 0) - (a.lastPlayed || 0));
    const instanceViewActive = document.body.dataset.activeView === 'instance';

    rail.innerHTML = ordered.map((p) => `
      <button class="rail-instance ${instanceViewActive && p.id === state.profile?.id ? 'active' : ''}"
              data-id="${esc(p.id)}" data-tip-text="${esc(p.name)}" aria-label="${esc(p.name)}">
        ${coverHtml(p)}
        ${state.running.has(p.id) ? '<span class="running-dot"></span>' : ''}
      </button>`).join('');

    $$('.rail-instance', rail).forEach((btn) => {
      btn.onpointerenter = () => showRailInstanceTooltip(btn);
      btn.onpointerleave = hideRailInstanceTooltip;
      btn.onfocus = () => showRailInstanceTooltip(btn);
      btn.onblur = hideRailInstanceTooltip;
      btn.onclick = () => {
        hideRailInstanceTooltip();
        openInstance(btn.dataset.id);
      };
    });
    rail.onscroll = hideRailInstanceTooltip;
  }

  function updatePlayButton() {
    const running = state.profile && state.running.has(state.profile.id);
    const busy = ['preparing', 'java', 'loader', 'downloading', 'account', 'launching'].includes(state.launchState);

    // Start and the instance page each have one, and they must never disagree.
    for (const [btnSel, labelSel] of [['#btn-play', '#play-label'], ['#ins-play', '#ins-play-label']]) {
      const btn = $(btnSel);
      const label = $(labelSel);
      if (!btn || !label) continue;
      btn.disabled = busy;
      btn.classList.toggle('brand', !running);
      label.textContent = running ? t('btn.stop') : busy ? t('btn.preparing') : t('btn.play');
      const glyph = $('.ic', btn);
      if (glyph) glyph.outerHTML = icon(running ? 'stop' : 'play');
    }

    const profileTrigger = $('#btn-home-profile');
    if (profileTrigger) {
      profileTrigger.disabled = busy || Boolean(running) || state.profiles.length === 0;
      profileTrigger.classList.toggle('brand', !running);
      $('.home-play-wrap')?.classList.toggle('running', Boolean(running));
      if (profileTrigger.disabled) closeHomeProfileMenu();
    }
  }

  /** Running instances live in the statusbar, next to the console button. */
  /**
   * The switch in the statusbar. It follows the selected profile, because
   * MangoConfig is per instance - the global setting only decides what a
   * profile does when it has no opinion of its own.
   */
  async function renderMangoConfigPill() {
    const pill = $('#mc-pill');
    if (!pill) return;
    if (!state.profile) { pill.hidden = true; return; }

    let info;
    try {
      info = await api.mangoConfig.info(state.profile.id);
    } catch {
      pill.hidden = true;
      return;
    }
    state.mangoConfig = info;

    pill.hidden = false;
    pill.classList.toggle('on', info.enabled && info.supported);
    pill.classList.toggle('off', !info.enabled);
    pill.classList.toggle('unsupported', !info.supported);
    pill.setAttribute('aria-pressed', String(Boolean(info.enabled && info.supported)));
    pill.setAttribute('aria-label', `${t('mangoconfig.title')}: ${info.enabled
      ? (info.supported ? t('mangoconfig.on') : t('mangoconfig.na'))
      : t('mangoconfig.off')}`);
    $('#mc-state').innerHTML = info.enabled && info.supported ? icon('check') : '';
    // Fourteen versions would burst the tooltip, so a span reads better.
    const versions = info.gameVersions.length > 3
      ? `${info.gameVersions[0]} – ${info.gameVersions[info.gameVersions.length - 1]}`
      : info.gameVersions.join(', ');
    pill.dataset.tipText = info.supported
      ? t('mangoconfig.tip')
      : t('mangoconfig.unsupported', { versions });
  }

  $('#mc-pill').onclick = async () => {
    if (!state.profile) return;
    const on = state.mangoConfig?.enabled;
    // null puts the instance back under the global setting, which is on.
    await api.profiles.update(state.profile.id, { mangoConfig: on ? false : null });
    await refreshState();
    toast(on ? t('mangoconfig.turnedOff') : t('mangoconfig.turnedOn'), 'ok');
  };

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
    $('#account-chip').setAttribute('aria-label', account ? account.name : t('account.none'));

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
    state.accounts.slice(0, 8).forEach((a) => loadSkin(a.uuid));

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

  function renderHomeProfileMenu() {
    const menu = $('#home-profile-menu');
    menu.innerHTML = state.profiles.map((profile) => {
      const selected = profile.id === state.profile?.id;
      const meta = `${profile.mcVersion} · ${LOADER_LABEL[profile.loader] || profile.loader}`;
      return `<button class="home-profile-option ${selected ? 'selected' : ''}"
                      data-profile-id="${esc(profile.id)}" role="menuitemradio"
                      aria-checked="${selected}">
        ${coverHtml(profile)}
        <span class="home-profile-option-copy">
          <span class="home-profile-option-name">${esc(profile.name)}</span>
          <span class="home-profile-option-meta">${esc(meta)}</span>
        </span>
        ${selected ? `<span class="home-profile-option-check">${icon('check')}</span>` : ''}
      </button>`;
    }).join('');
    hydrateIcons(menu);

    $$('.home-profile-option', menu).forEach((option) => {
      option.onclick = async (event) => {
        event.stopPropagation();
        option.disabled = true;
        try {
          if (option.dataset.profileId !== state.profile?.id) {
            await api.profiles.select(option.dataset.profileId);
            await refreshState();
          }
        } catch (err) {
          toast(err.message, 'err');
        } finally {
          closeHomeProfileMenu();
        }
      };
    });
  }

  $('#btn-home-profile').onclick = (event) => {
    event.stopPropagation();
    const trigger = $('#btn-home-profile');
    const menu = $('#home-profile-menu');
    const opening = menu.hidden;
    $('#account-menu').hidden = true;
    if (opening) renderHomeProfileMenu();
    menu.hidden = !opening;
    trigger.setAttribute('aria-expanded', String(opening));
    if (opening) $('.home-profile-option.selected', menu)?.focus();
  };
  $('#home-profile-menu').onclick = (event) => event.stopPropagation();

  $('#btn-edit-current').onclick = () => state.profile && openProfileDialog(state.profile);
  $('#btn-start-add-mods').onclick = () => openContentBrowser('mod');
  $('#btn-start-mods-folder').onclick = () => state.profile && api.app.openFolder(state.profile.id);
  $('#btn-console').onclick = () => { $('#console-drawer').hidden = false; scrollConsole(); };
  $('#btn-close-console').onclick = () => { $('#console-drawer').hidden = true; };
  $('#btn-clear-console').onclick = () => {
    state.consoleLines = [];
    consoleBytes = 0;
    $('#console-out').innerHTML = '';
  };
  $('#btn-kill').onclick = async () => {
    for (const id of state.running) await api.game.stop(id).catch(() => {});
  };

  async function playOrStop() {
    if (!state.profile) { showView('profiles'); return; }
    if (state.running.has(state.profile.id)) {
      await api.game.stop(state.profile.id).catch(() => {});
      return;
    }
    if (!state.account) { toast(t('launch.needAccount'), 'err'); showView('accounts'); return; }
    await doLaunch(state.profile.id);
  }

  $('#btn-play').onclick = () => playOrStop();

  async function doLaunch(profileId, quickJoin) {
    state.launchState = 'preparing';
    updatePlayButton();
    setLaunchProgressVisible(true);
    setProgress(0, t('launch.preparing'));
    try {
      await api.game.launch(profileId, quickJoin);
    } catch (err) {
      state.launchState = 'idle';
      setLaunchProgressVisible(false);
      updatePlayButton();
      toast(err.message, 'err');
      if (err.needsRelogin) showView('accounts');
      $('#console-drawer').hidden = false;
      pushConsole(`Launch failed: ${err.message}`, 'error');
    }
  }

  function setLaunchProgressVisible(visible) {
    $$('[data-launch-progress]').forEach((progress) => { progress.hidden = !visible; });
  }

  function setProgress(pct, label) {
    $$('[data-progress-fill]').forEach((fill) => {
      fill.style.width = `${Math.max(0, Math.min(100, pct))}%`;
    });
    $$('[data-progress-pct]').forEach((value) => { value.textContent = `${Math.round(pct)}%`; });
    $$('[data-progress-text]').forEach((value) => { value.textContent = label; });
  }

  // =========================================================================
  // skin viewer
  // =========================================================================

  function ensureViewer() {
    if (viewerUnavailable) return null;
    if (viewer) return viewer;
    const canvas = $('#skin-canvas');
    if (!canvas) return null;
    try {
      viewer = new window.SkinViewer(canvas, { slim: false });
    } catch (err) {
      console.warn('SkinViewer unavailable:', err);
      viewerUnavailable = true;
      canvas.replaceWith(Object.assign(document.createElement('div'), {
        className: 'empty', textContent: t('skin.noWebgl'),
      }));
    }
    return viewer;
  }

  function ensureHomeViewer() {
    if (homeViewerUnavailable) return null;
    if (homeViewer) return homeViewer;
    const canvas = $('#home-skin-canvas');
    if (!canvas) return null;
    try {
      homeViewer = new window.SkinViewer(canvas, { slim: false, rotation: -0.35 });
    } catch (err) {
      console.warn('Home SkinViewer unavailable:', err);
      homeViewerUnavailable = true;
      canvas.hidden = true;
    }
    return homeViewer;
  }

  function destroyViewer() {
    if (!viewer) return;
    const canvas = viewer.canvas;
    viewer.destroy();
    viewer = null;
    viewerSkin = null;
    // A WebGL context belongs to its canvas for life. Replace the hidden canvas
    // so reopening Accounts receives a fresh context instead of a lost one.
    if (canvas?.isConnected) canvas.replaceWith(canvas.cloneNode(false));
  }

  function destroyHomeViewer() {
    if (!homeViewer) return;
    const canvas = homeViewer.canvas;
    homeViewer.destroy();
    homeViewer = null;
    homeViewerSkin = null;
    if (canvas?.isConnected) canvas.replaceWith(canvas.cloneNode(false));
  }

  function syncViewer() {
    const skin = state.account ? state.skins.get(state.account.uuid) : state.defaultSkin;
    if (!skin) return;

    const accountsVisible = !document.hidden && $('#view-accounts').classList.contains('active');
    if (!accountsVisible) destroyViewer();
    else {
      const activeViewer = ensureViewer();
      if (activeViewer && viewerSkin !== skin) {
        activeViewer.setSkin(skin.skin, skin.slim);
        viewerSkin = skin;
      }
    }

    const homeVisible = !document.hidden && $('#view-start').classList.contains('active');
    if (!homeVisible) destroyHomeViewer();
    else {
      const activeHomeViewer = ensureHomeViewer();
      if (activeHomeViewer && homeViewerSkin !== skin) {
        activeHomeViewer.setSkin(skin.skin, skin.slim);
        homeViewerSkin = skin;
      }
    }
  }

  document.addEventListener('visibilitychange', syncViewer);

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
      state.defaultSkin = skin;
      state.defaultHead = await makeHead(skin.skin);
      if (!state.account) {
        syncViewer();
        renderAccountChip();
      }
    } catch { /* WebGL or IPC unavailable, the stage just stays empty */ }
  }

  async function loadSkin(uuid) {
    if (!uuid || state.skins.has(uuid) || loadingSkins.has(uuid)) return;
    loadingSkins.add(uuid);
    try {
      await applySkin(uuid, await api.skins.get(uuid));
    } catch (err) {
      console.warn('skin load failed', err);
    } finally {
      loadingSkins.delete(uuid);
    }
  }

  async function applySkin(uuid, skin) {
    skin.head = await makeHead(skin.skin);
    state.skins.set(uuid, skin);
    if (state.account?.uuid === uuid) syncViewer();
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
      // Clicking the tile opens the instance's own page.
      card.onclick = (e) => {
        if (e.target.closest('[data-act]')) return;
        openInstance(id);
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
      { key: 'mods', label: t('profiles.manageContent'), glyph: 'package' },
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
    if (!state.versions.versions.length) {
      versionsPromise ||= api.versions.manifest().then((manifest) => {
        state.versions = manifest;
      }).catch((err) => {
        console.warn('version manifest failed', err);
        state.versions = {
          versions: [{ id: state.profile?.mcVersion || '1.21.11', type: 'release' }],
          latest: {},
          installed: [],
        };
      });
      await versionsPromise;
    }
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
    state.accounts.forEach((a) => loadSkin(a.uuid));
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

  $('#btn-add-offline').onclick = () => openOfflineDialog();

  /** Ask for an offline name and add the account. `onClose` always runs. */
  function openOfflineDialog(onClose) {
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
      onClose: () => onClose?.(),
      onOpen: (body) => {
        const input = $('#off-name', body);
        input.focus();
        input.onkeydown = (e) => { if (e.key === 'Enter') $('#modal-foot .brand').click(); };
      },
    });
  }

  // =========================================================================
  // mods
  // =========================================================================

  const QUICK_PICKS = ['sodium', 'lithium', 'iris', 'fabric-api'];
  const PERF_PACK = ['sodium', 'lithium', 'ferrite-core', 'entityculling', 'modernfix', 'immediatelyfast'];

  function selectModPageTab(tab, render = true) {
    $$('#mod-tabs .tab').forEach((button) => button.classList.toggle('active', button.dataset.tab === tab));
    $('#mods-browse').hidden = tab !== 'browse';
    $('#mods-installed').hidden = tab !== 'installed';
    if (!render) return;
    if (tab === 'installed') { renderInstalledMods(); syncLocalMods(); }
    else renderMods();
  }

  $$('#mod-tabs .tab').forEach((btn) => {
    btn.onclick = () => selectModPageTab(btn.dataset.tab);
  });

  /** Open Modrinth with the content kind already selected for this instance. */
  function openContentBrowser(type = 'mod') {
    const selected = CONTENT_TYPES.includes(type) ? type : 'mod';
    $('#mod-type').value = selected;
    state.modSearch.offset = 0;
    selectModPageTab('browse', false);
    if (history.stack[history.idx] === 'mods') renderMods();
    else showView('mods');
  }

  let searchTimer = null;
  let modSearchToken = 0;
  $('#mod-search').oninput = () => {
    clearTimeout(searchTimer);
    searchTimer = setTimeout(() => { state.modSearch.offset = 0; renderMods(); }, 220);
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
    const token = ++modSearchToken;
    if (!state.profile) {
      $('#mod-grid').innerHTML = `<div class="empty"><div class="et">${esc(t('profiles.none'))}</div>${esc(t('mods.needProfile'))}</div>`;
      return;
    }
    if ($('#mod-type').value === 'mod') renderQuickPicks();
    else $('#quickpicks').replaceChildren();
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
      if (token !== modSearchToken || !$('#view-mods').classList.contains('active')) return;

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
      if (token !== modSearchToken || !$('#view-mods').classList.contains('active')) return;
      grid.innerHTML = `<div class="empty"><div class="et">${esc(t('mods.searchFailed'))}</div>${esc(err.message)}</div>`;
    }
  }

  async function installMod(slug, btn, versionId = null) {
    if (!state.profile) return;
    const original = btn?.textContent;
    if (btn) { btn.disabled = true; btn.textContent = t('mods.installing'); }
    try {
      const profileId = state.profile.id;
      const installed = await api.modrinth.install(profileId, slug, versionId);
      clearModUpdate(profileId, installed[0]?.projectId || slug);
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
   * Jars can be dropped into the mods folder by hand, so the list has to be
   * re-read from disk rather than trusted from the last install.
   */
  function applyMods(profileId, mods) {
    const profile = state.profiles.find((p) => p.id === profileId);
    if (profile) profile.mods = mods;
    if (state.profile?.id === profileId) state.profile.mods = mods;
    renderStart();
    if ($('#view-mods').classList.contains('active') && !$('#mods-installed').hidden) renderInstalledMods();
  }

  async function syncLocalMods() {
    if (!state.profile) return;
    const id = state.profile.id;
    try {
      applyMods(id, await api.mods.sync(id));
    } catch (err) {
      console.warn('mod folder sync failed', err);
    }
  }

  /**
   * Shared installed-content list. A type can be supplied for an instance's
   * Mods / Resource Packs / Shaders shelves; without one it shows everything.
   */
  function renderModList(list, type = null) {
    if (!list) return;
    const mods = profileContent(state.profile, type);
    if (!mods.length) {
      const emptyKey = type ? `content.none.${type}` : 'mods.none';
      const hintKey = type ? `content.noneHint.${type}` : 'mods.noneHint';
      list.innerHTML = `<div class="empty content-empty"><div class="empty-icon" data-icon="${CONTENT_ICONS[type] || 'package'}"></div><div class="et">${esc(t(emptyKey))}</div>${esc(t(hintKey))}</div>`;
      hydrateIcons(list);
      return;
    }
    list.innerHTML = mods.map((m) => {
      const owners = dependencyOwners(state.profile, m.projectId);
      const dependencyHint = owners.length
        ? t('mods.dependencyHint', { names: owners.map((owner) => owner.title).join(', ') })
        : t('mods.dependencyHintUnknown');
      return `
      <div class="row content-row ${m.enabled === false ? 'off' : ''}" data-id="${esc(m.projectId)}">
        ${m.icon ? `<img class="mod-icon" src="${esc(m.icon)}" alt="" />` : `<span class="mod-icon" data-icon="${CONTENT_ICONS[contentType(m)] || 'package'}"></span>`}
        <div class="row-meta">
          <div class="row-name">${esc(m.title)}
            ${contentType(m) !== 'mod' ? `<span class="badge content-kind">${esc(contentLabel(contentType(m)))}</span>` : ''}
            ${m.dependency ? `<span class="badge dependency-badge" tabindex="0" title="${esc(dependencyHint)}" data-tip-text="${esc(dependencyHint)}" aria-label="${esc(dependencyHint)}">${esc(t('mods.dependency'))}</span>` : ''}
            ${m.local ? `<span class="badge" data-tip-text="${esc(t('mods.localHint'))}">${esc(t('mods.local'))}</span>` : ''}
            ${m.update ? `<span class="badge warn"${m.update.conflict ? ` data-tip-text="${esc(t('mods.conflictHint', { names: (m.update.requiredBy || []).map((item) => item.title).join(', ') }))}"` : ''}>${esc(t(m.update.conflict ? 'mods.dependencyConflict' : m.update.compatibility ? 'mods.compatibilityFix' : 'mods.updateAvailable'))}</span>` : ''}
          </div>
          <div class="row-sub">${m.versionNumber ? `${esc(m.versionNumber)} · ` : ''}${esc(m.filename)}</div>
        </div>
        <span class="row-state">${esc(m.enabled === false ? t('mods.off') : t('mods.on'))}</span>
        <label class="switch" data-tip-text="${esc(t('mods.toggle'))}">
          <input type="checkbox" data-act="toggle" aria-label="${esc(t('mods.toggle'))}" ${m.enabled === false ? '' : 'checked'} /><span class="slider"></span>
        </label>
        ${m.update && !m.update.conflict ? `<button class="btn sm brand" data-act="update">${esc(t(m.update.compatibility ? 'mods.fix' : 'mods.update'))}</button>` : ''}
        <button class="btn sm icon-only danger" data-act="remove" data-icon="trash"
                data-tip-text="${esc(t('mods.remove'))}" aria-label="${esc(t('mods.remove'))}"></button>
      </div>`;
    }).join('');
    hydrateIcons(list);

    $$('.row', list).forEach((row) => {
      const id = row.dataset.id;
      const mod = (state.profile.mods || []).find((m) => m.projectId === id);

      $('[data-act="toggle"]', row).onchange = async (e) => {
        const enabled = e.target.checked;
        try {
          const result = await api.modrinth.toggle(state.profile.id, id, enabled);
          await refreshState();
          const cascaded = (result.affected || []).filter((item) => item.projectId !== id);
          if (cascaded.length) {
            toast(t(enabled ? 'mods.dependenciesEnabled' : 'mods.dependentsDisabled', {
              n: cascaded.length,
              names: cascaded.map((item) => item.title).join(', '),
            }), 'info');
          }
        } catch (err) {
          e.target.checked = !enabled; // put the switch back where it was
          toast(err.message, 'err');
        }
      };

      $('[data-act="remove"]', row).onclick = () => {
        confirmDialog({
          title: t('content.deleteTitle'),
          text: t('content.deleteConfirm', { name: mod?.title || id }),
          confirmLabel: t('btn.delete'),
          onConfirm: async () => {
            try {
              const profileId = state.profile.id;
              const result = await api.modrinth.uninstall(profileId, id);
              if (result.blocked) {
                toast(t('mods.removeBlocked', {
                  names: result.dependents.map((item) => item.title).join(', '),
                }), 'err');
                return;
              }
              clearModUpdate(profileId, id);
              await refreshState();
              toast(t('content.deleted', { name: mod?.title || id }), 'ok');
            } catch (err) { toast(err.message, 'err'); }
          },
        });
      };

      const upd = $('[data-act="update"]', row);
      if (upd) {
        upd.onclick = async () => {
          await installMod(id, upd, mod?.update?.versionId || null);
          if (mod) delete mod.update;
          renderModList(list, type);
        };
      }
    });
  }

  function clearModUpdate(profileId, projectId) {
    const updates = state.modUpdates.get(profileId);
    if (!updates) return;
    state.modUpdates.set(profileId, updates.filter((update) => update.projectId !== projectId));
  }

  /** Mark which installed content has a newer or compatibility-required build. */
  function applyModUpdates(updates, profileId = state.profile?.id) {
    if (!profileId) return;
    state.modUpdates.set(profileId, updates);
    const targets = [state.profiles.find((profile) => profile.id === profileId),
      state.profile?.id === profileId ? state.profile : null].filter(Boolean);
    for (const profile of new Set(targets)) {
      for (const mod of profile.mods || []) {
        const hit = updates.find((update) => update.projectId === mod.projectId);
        if (hit) mod.update = hit; else delete mod.update;
      }
    }
  }

  async function checkModUpdatesOnStartup() {
    try {
      const checked = await api.modrinth.checkAllUpdates();
      // The main process also hydrates dependency graphs for older profiles.
      // Pull those records in before rendering owner names on dependency badges.
      await refreshState();
      let total = 0;
      for (const result of checked) {
        applyModUpdates(result.updates || [], result.profileId);
        total += result.updates?.length || 0;
        if (result.error) console.warn(`mod update check failed for ${result.profileId}: ${result.error}`);
      }
      renderAll();
      if (total) toast(t('mods.autoUpdatesFound', { n: total }), 'info');
    } catch (err) {
      console.warn('automatic mod update check failed', err);
    }
  }

  /** Fill the three default browser queries after the shell is usable. Opening
   * Mods, resource packs or shaders then usually renders from the main-process
   * cache instead of waiting for the first Modrinth round trip. */
  async function warmContentSearch() {
    if (!state.profile) return;
    await Promise.allSettled(CONTENT_TYPES.map((projectType) => api.modrinth.search({
      query: '',
      loader: state.profile.loader,
      gameVersion: state.profile.mcVersion,
      projectType,
      index: 'relevance',
      limit: 30,
      offset: 0,
    })));
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
      applyModUpdates(updates);
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
        <span class="server-icon-action">
          ${serverIconHtml(s)}
          <span class="server-play-overlay">${icon('play')}</span>
        </span>
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

  /** Settings use a section rail so global preferences stay scannable. */
  const SETTINGS_SECTIONS = [
    { key: 'game', glyph: 'monitor' },
    { key: 'performance', glyph: 'gauge' },
    { key: 'java', glyph: 'coffee' },
    { key: 'launcher', glyph: 'window' },
    { key: 'storage', glyph: 'database' },
    { key: 'updates', glyph: 'download' },
  ];

  function renderSettingsNav() {
    const nav = $('#settings-nav');
    nav.innerHTML = SETTINGS_SECTIONS.map((s) => `
      <button data-section="${s.key}">
        <span data-icon="${s.glyph}"></span>${esc(t(`settings.section.${s.key}`))}
      </button>`).join('');
    hydrateIcons(nav);
    $$('button', nav).forEach((b) => { b.onclick = () => showSettingsSection(b.dataset.section); });
  }

  function showSettingsSection(key) {
    state.settingsSection = key;
    $$('#settings-nav button').forEach((b) => b.classList.toggle('active', b.dataset.section === key));
    $$('#settings-grid .settings-card').forEach((c) => { c.hidden = c.dataset.section !== key; });
    // Walking the folder tree is not free, so only do it when it is on screen.
    if (key === 'storage') loadStorageUsage();
  }

  async function loadStorageUsage() {
    const list = $('#storage-list');
    if (!list) return;
    list.innerHTML = '<div class="loading-row">…</div>';

    let usage;
    try {
      usage = await api.storage.usage();
    } catch (err) {
      list.textContent = err.message;
      return;
    }
    if (!$('#storage-list')) return; // the player left the section meanwhile

    $('#storage-total').textContent = fmtBytes(usage.total);
    $('#storage-list').innerHTML = usage.sections.map((s) => `
      <div class="storage-row">
        <div class="storage-name">${esc(t(`storage.${s.key}`))}<small>${esc(t(`storage.${s.key}D`))}</small></div>
        <span class="storage-size">${esc(fmtBytes(s.bytes))}</span>
        ${s.removable ? `<button class="btn sm danger" data-clear="${s.key}">${esc(t('settings.storageClear'))}</button>` : ''}
      </div>`).join('');

    $$('[data-clear]', $('#storage-list')).forEach((btn) => {
      btn.onclick = () => clearStorage(btn.dataset.clear);
    });
  }

  function clearStorage(key) {
    const name = t(`storage.${key}`);
    confirmDialog({
      title: name,
      text: t('settings.storageClearConfirm', { name }),
      confirmLabel: t('settings.storageClear'),
      onConfirm: async () => {
        try {
          await api.storage.clear(key);
          toast(t('settings.storageCleared', { name }), 'ok');
          loadStorageUsage();
        } catch (err) {
          toast(err.message, 'err');
        }
      },
    });
  }

  function renderSettings() {
    const c = state.config;
    $('#settings-grid').innerHTML = `
      <div class="settings-card" data-section="game">
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

      <div class="settings-card" data-section="performance">
        <h2><span data-icon="gauge"></span>${esc(t('settings.performance'))}</h2>
        <div class="sc-sub">${esc(t('settings.performanceSub'))}</div>
        <div class="preset-row" id="set-presets">
          ${['potato', 'balanced', 'quality'].map((key) => `
            <button class="preset ${c.performancePreset === key ? 'active' : ''}" data-preset="${key}">
              <span class="pn">${esc(t(`settings.preset.${key}`))}</span>
              <span class="pd">${esc(t(`settings.preset.${key}D`))}</span>
            </button>`).join('')}
        </div>
        <div class="toggle-row" style="margin-top:12px">
          <div><div class="toggle-label">${esc(t('settings.perfMods'))}</div><div class="toggle-desc">${esc(t('settings.perfModsDesc'))}</div></div>
          <label class="switch"><input type="checkbox" id="set-perfmods" ${c.performanceMods !== false ? 'checked' : ''} /><span class="slider"></span></label>
        </div>
        <div class="field" style="margin-top:16px">
          <label>${esc(t('settings.downloads'))}</label>
          <input type="number" id="set-dl" value="${c.concurrentDownloads}" min="1" max="32" />
          <div class="hint">${esc(t('settings.downloadsHint'))}</div>
        </div>
      </div>

      <div class="settings-card" data-section="java">
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

      <div class="settings-card" data-section="launcher">
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
        <div class="toggle-row">
          <div><div class="toggle-label">${esc(t('mangoconfig.title'))}</div><div class="toggle-desc">${esc(t('mangoconfig.desc'))}</div></div>
          <label class="switch"><input type="checkbox" id="set-mangoconfig" ${c.mangoConfig !== false ? 'checked' : ''} /><span class="slider"></span></label>
        </div>
      </div>

      <div class="settings-card" data-section="storage">
        <h2><span data-icon="database"></span>${esc(t('settings.storage'))}</h2>
        <div class="sc-sub">${esc(t('settings.storageSub'))}</div>
        <div class="field"><input readonly value="${esc(state.paths?.root || '')}" /></div>
        <div class="field-row">
          <button class="btn" id="btn-open-root" data-icon="folder"><span>${esc(t('settings.openRoot'))}</span></button>
          <button class="btn" id="btn-open-logs" data-icon="folder"><span>${esc(t('settings.openLogs'))}</span></button>
        </div>
        <div class="toggle-row" style="margin-top:14px">
          <div class="toggle-label">${esc(t('settings.storageTotal'))}</div>
          <b id="storage-total">…</b>
        </div>
        <div id="storage-list"></div>
      </div>

      <div class="settings-card" data-section="updates">
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
    $('#settings-grid').classList.add('sectioned');
    hydrateIcons($('#settings-grid'));
    renderSettingsNav();
    showSettingsSection(state.settingsSection);

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
    $('#set-perfmods').onchange = (e) => save({ performanceMods: e.target.checked });
    $('#set-mangoconfig').onchange = async (e) => {
      await save({ mangoConfig: e.target.checked });
      renderMangoConfigPill();
    };
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
  // instance page
  //
  // Every instance owns a tabbed workspace. The page always shows the selected
  // profile, so the rail, Start view and this view never disagree.
  // =========================================================================

  const INSTANCE_TABS = ['overview', 'mods', 'shots', 'logs', 'settings'];

  async function openInstance(profileId, tab = 'overview') {
    if (profileId && profileId !== state.profile?.id) {
      await api.profiles.select(profileId);
      await refreshState();
    }
    state.instanceTab = INSTANCE_TABS.includes(tab) ? tab : 'overview';
    // showView() ignores a repeat of the current view, but switching instances
    // does have to redraw, so say it plainly.
    if (history.stack[history.idx] === 'instance') renderView('instance');
    else showView('instance');
  }

  function renderInstance() {
    const profile = state.profile;
    if (!profile) { showView('profiles'); return; }

    const tile = $('#ins-tile');
    tile.setAttribute('style', coverStyle(profile));
    tile.textContent = coverLetter(profile);
    $('#ins-name').textContent = profile.name;
    $('#ins-sub').textContent = `${profile.mcVersion} · ${LOADER_LABEL[profile.loader] || profile.loader}`;
    $('#ins-facts').innerHTML = factsHtml(profile, [
      [t('instance.playTime'), fmtDuration(profile.playTimeMs || 0)],
      [t('instance.created'), fmtDate(profile.created)],
    ]);
    updatePlayButton();
    showInstanceTab(state.instanceTab || 'overview');
  }

  function showInstanceTab(tab) {
    state.instanceTab = tab;
    $$('#ins-tabs .tab').forEach((b) => b.classList.toggle('active', b.dataset.itab === tab));
    $$('#view-instance .tabpanel').forEach((p) => {
      const active = p.id === `itab-${tab}`;
      p.classList.toggle('active', active);
      if (!active) p.replaceChildren();
    });
    if (tab !== 'shots') state.shots = [];
    if (tab !== 'logs') { state.logs = []; state.logId = null; }

    if (tab === 'overview') renderInstanceOverview();
    if (tab === 'mods') renderInstanceMods();
    if (tab === 'shots') renderShots();
    if (tab === 'logs') renderInstanceLogs();
    if (tab === 'settings') renderInstanceSettings();
  }

  $$('#ins-tabs .tab').forEach((btn) => { btn.onclick = () => showInstanceTab(btn.dataset.itab); });
  $('#ins-edit').onclick = () => state.profile && openProfileDialog(state.profile);
  $('#ins-folder').onclick = () => state.profile && api.app.openFolder(state.profile.id);
  $('#ins-play').onclick = () => playOrStop();

  // --- overview ------------------------------------------------------------

  function renderInstanceOverview() {
    const profile = state.profile;
    if (!profile) return;
    const panel = $('#itab-overview');
    panel.innerHTML = `
      <section class="panel">
        <div class="panel-head"><h2>${esc(t('instance.quick'))}</h2></div>
        <div class="rows">
          <button class="row menu-row" data-act="browse">
            <span data-icon="compass"></span><span class="row-name">${esc(t('instance.selectMods'))}</span></button>
          <button class="row menu-row" data-act="shots">
            <span data-icon="image"></span><span class="row-name">${esc(t('instance.openShots'))}</span></button>
          <button class="row menu-row" data-act="logs">
            <span data-icon="file"></span><span class="row-name">${esc(t('instance.openLogs'))}</span></button>
        </div>
      </section>

      <section class="panel">
        <div class="panel-head">
          <h2>${esc(t('instance.recentShots'))}</h2>
          <button class="btn sm" data-act="allshots">${esc(t('servers.all'))}</button>
        </div>
        <div class="rows"><div class="shot-grid" id="ov-shots"></div></div>
      </section>

      <section class="panel">
        <div class="panel-head">
          <h2>${esc(t('mods.installed'))} <span class="count">${profileContent(profile, 'mod').length || ''}</span></h2>
          <button class="btn sm" data-act="allmods">${esc(t('servers.all'))}</button>
        </div>
        <div class="rows" id="ov-mods"></div>
      </section>`;
    hydrateIcons(panel);

    $('[data-act="browse"]', panel).onclick = () => openContentBrowser('mod');
    $('[data-act="shots"]', panel).onclick = () => api.screenshots.openFolder(profile.id).catch((err) => toast(err.message, 'err'));
    $('[data-act="logs"]', panel).onclick = () => api.logs.openFolder(profile.id).catch((err) => toast(err.message, 'err'));
    $('[data-act="allshots"]', panel).onclick = () => showInstanceTab('shots');
    $('[data-act="allmods"]', panel).onclick = () => showInstanceTab('mods');

    renderModList($('#ov-mods'), 'mod');

    // The strip is a preview, so a slow folder must never hold up the page.
    api.screenshots.list(profile.id).then((shots) => {
      state.shots = shots;
      const strip = $('#ov-shots');
      if (!strip) return; // the player moved on while we read the folder
      if (!shots.length) {
        strip.innerHTML = `<div class="empty"><div class="et">${esc(t('shots.none'))}</div>${esc(t('shots.noneHint'))}</div>`;
        return;
      }
      strip.innerHTML = shots.slice(0, 4).map((s, i) => shotHtml(s, i)).join('');
      bindShots(strip);
    }).catch(() => {});
  }

  // --- instance content ----------------------------------------------------

  function renderInstanceMods() {
    const profile = state.profile;
    if (!profile) return;
    const panel = $('#itab-mods');
    const selected = CONTENT_TYPES.includes(state.instanceContentType) ? state.instanceContentType : 'mod';
    state.instanceContentType = selected;
    const addLabel = t(`content.add.${selected}`);
    panel.innerHTML = `
      <div class="content-switcher" role="tablist" aria-label="${esc(t('content.tabs'))}">
        ${CONTENT_TYPES.map((type) => `<button class="content-switch ${type === selected ? 'active' : ''}"
          data-content-type="${type}" role="tab" aria-selected="${type === selected}">
          <span data-icon="${CONTENT_ICONS[type]}"></span>
          <span>${esc(contentLabel(type))}</span>
          <span class="content-count">${profileContent(profile, type).length}</span>
        </button>`).join('')}
      </div>
      <div class="tab-toolbar">
        <button class="btn brand" data-act="browse" data-icon="plus"><span>${esc(addLabel)}</span></button>
        <button class="btn" data-act="updates" data-icon="refresh"><span>${esc(t('mods.checkUpdates'))}</span></button>
        <span class="spacer"></span>
        <button class="btn" data-act="folder" data-icon="folder"><span>${esc(t('content.openFolder'))}</span></button>
      </div>
      <div class="rows" id="ins-mod-list"></div>`;
    hydrateIcons(panel);

    $$('[data-content-type]', panel).forEach((button) => {
      button.onclick = () => {
        state.instanceContentType = button.dataset.contentType;
        renderInstanceMods();
      };
    });
    $('[data-act="browse"]', panel).onclick = () => openContentBrowser(selected);
    $('[data-act="folder"]', panel).onclick = () => api.app.openContentFolder(profile.id, selected);
    $('[data-act="updates"]', panel).onclick = async (e) => {
      const btn = e.currentTarget;
      btn.disabled = true;
      try {
        const updates = await api.modrinth.checkUpdates(profile.id);
        applyModUpdates(updates);
        const relevant = updates.filter((update) => (update.type || 'mod') === selected);
        renderModList($('#ins-mod-list'), selected);
        toast(relevant.length ? t('mods.updatesFound', { n: relevant.length }) : t('mods.upToDate'),
          relevant.length ? 'info' : 'ok');
      } catch (err) {
        toast(err.message, 'err');
      } finally {
        btn.disabled = false;
      }
    };

    renderModList($('#ins-mod-list'), selected);
  }

  // --- per-instance settings ----------------------------------------------

  function renderInstanceSettings() {
    const profile = state.profile;
    if (!profile) return;
    const panel = $('#itab-settings');
    panel.innerHTML = `
      <div class="settings-grid sectioned">
        <div class="settings-card">
          <h2><span data-icon="gauge"></span>${esc(t('settings.performance'))}</h2>
          <div class="sc-sub">${esc(t('instance.settingsSub'))}</div>
          <div class="toggle-row">
            <div>
              <div class="toggle-label">${esc(t('instance.ramOverride'))}</div>
              <div class="toggle-desc">${esc(t('instance.ramInherit', { value: fmtGB(state.config.ram || 4096) }))}</div>
            </div>
            <label class="switch">
              <input type="checkbox" id="ins-ram-on" ${profile.ram ? 'checked' : ''} /><span class="slider"></span>
            </label>
          </div>
          <div class="field" id="ins-ram-field" ${profile.ram ? '' : 'hidden'}>
            <div class="range-head"><label>${esc(t('settings.ram'))}</label><b id="ins-ram-label">${esc(fmtGB(profile.ram || state.config.ram || 4096))}</b></div>
            <input type="range" id="ins-ram" min="1024" max="${state.totalRam}" step="512" value="${profile.ram || state.config.ram || 4096}" />
          </div>
          <div class="field">
            <label>${esc(t('instance.argsOverride'))}</label>
            <input id="ins-args" value="${esc(profile.javaArgs || '')}" placeholder="${esc(state.config.javaArgs || '-XX:+UseG1GC')}" />
          </div>
        </div>

        <div class="settings-card">
          <h2><span data-icon="sliders"></span>${esc(t('mangoconfig.title'))}</h2>
          <div class="sc-sub">${esc(t('mangoconfig.instanceSub'))}</div>
          <div class="toggle-row">
            <div>
              <div class="toggle-label">${esc(t('mangoconfig.perInstance'))}</div>
              <div class="toggle-desc">${esc(t(state.config.mangoConfig === false ? 'mangoconfig.globalOff' : 'mangoconfig.globalOn'))}</div>
            </div>
            <label class="switch">
              <input type="checkbox" id="ins-mangoconfig" ${profile.mangoConfig === false ? '' : 'checked'} /><span class="slider"></span>
            </label>
          </div>
        </div>

        <div class="settings-card">
          <h2><span data-icon="layers"></span>${esc(t('profiles.title'))}</h2>
          <div class="sc-sub">${esc(t('instance.dangerSub'))}</div>
          <div class="field-row">
            <button class="btn" id="ins-set-edit" data-icon="pencil"><span>${esc(t('profiles.edit'))}</span></button>
            <button class="btn" id="ins-set-dup" data-icon="copy"><span>${esc(t('profiles.duplicate'))}</span></button>
            <button class="btn danger" id="ins-set-del" data-icon="trash"><span>${esc(t('instance.dangerZone'))}</span></button>
          </div>
        </div>
      </div>`;
    hydrateIcons(panel);

    const save = (patch) => api.profiles.update(profile.id, patch).then(() => refreshState());

    const ram = $('#ins-ram');
    $('#ins-ram-on').onchange = (e) => {
      $('#ins-ram-field').hidden = !e.target.checked;
      save({ ram: e.target.checked ? Number(ram.value) : null });
    };
    ram.oninput = () => { $('#ins-ram-label').textContent = fmtGB(ram.value); };
    ram.onchange = () => save({ ram: Number(ram.value) });
    $('#ins-args').onchange = (e) => save({ javaArgs: e.target.value });
    // null rather than true, so the instance goes back to following the global setting.
    $('#ins-mangoconfig').onchange = async (e) => {
      await save({ mangoConfig: e.target.checked ? null : false });
      renderMangoConfigPill();
    };

    $('#ins-set-edit').onclick = () => openProfileDialog(profile);
    $('#ins-set-dup').onclick = async () => { await api.profiles.duplicate(profile.id); await refreshState(); };
    $('#ins-set-del').onclick = () => confirmDialog({
      title: t('profiles.deleteTitle'),
      text: t('profiles.deleteConfirm', { name: profile.name }),
      confirmLabel: t('btn.delete'),
      onConfirm: async () => {
        await api.profiles.remove(profile.id);
        await refreshState();
        showView('profiles');
      },
    });
  }

  // =========================================================================
  // screenshots
  //
  // The pictures are served over mangoimg://, a scheme the main process backs
  // with the instance folder; the renderer may not read files itself.
  // =========================================================================

  function shotHtml(shot, index) {
    return `<figure class="shot" role="button" tabindex="0" data-i="${index}">
      <img src="${esc(shot.url)}" alt="${esc(shot.name)}" loading="lazy" />
      <figcaption>${esc(shot.name)}</figcaption>
    </figure>`;
  }

  function bindShots(root) {
    $$('.shot', root).forEach((el) => {
      const open = () => openLightbox(Number(el.dataset.i));
      el.onclick = open;
      el.onkeydown = (e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); open(); } };
    });
  }

  async function renderShots() {
    const profile = state.profile;
    if (!profile) return;
    const panel = $('#itab-shots');
    panel.innerHTML = `
      <div class="tab-toolbar">
        <button class="btn" data-act="refresh" data-icon="refresh"><span>${esc(t('servers.refresh'))}</span></button>
        <button class="btn" data-act="folder" data-icon="folder"><span>${esc(t('instance.openShots'))}</span></button>
        <span class="spacer"></span>
        <span class="row-state" id="shot-count"></span>
      </div>
      <div class="shot-grid" id="shot-grid"></div>`;
    hydrateIcons(panel);

    $('[data-act="refresh"]', panel).onclick = () => renderShots();
    $('[data-act="folder"]', panel).onclick = () => api.screenshots.openFolder(profile.id).catch((err) => toast(err.message, 'err'));

    const grid = $('#shot-grid');
    try {
      state.shots = await api.screenshots.list(profile.id);
    } catch (err) {
      state.shots = [];
      toast(err.message, 'err');
    }
    if (state.instanceTab !== 'shots' || !$('#shot-grid')) return; // tab changed mid-read

    $('#shot-count').textContent = state.shots.length ? t('shots.count', { n: state.shots.length }) : '';
    if (!state.shots.length) {
      grid.innerHTML = `<div class="empty"><div class="et">${esc(t('shots.none'))}</div>${esc(t('shots.noneHint'))}</div>`;
      return;
    }
    grid.innerHTML = state.shots.map((s, i) => shotHtml(s, i)).join('');
    bindShots(grid);
  }

  // --- lightbox ------------------------------------------------------------

  function openLightbox(index) {
    const shot = state.shots[index];
    if (!shot) return;
    state.shotIndex = index;
    $('#lb-img').src = shot.url;
    $('#lb-name').textContent = shot.name;
    $('#lb-meta').textContent = `${fmtBytes(shot.size)} · ${fmtDateTime(shot.taken)}`;
    $('#lb-prev').disabled = index === 0;
    $('#lb-next').disabled = index >= state.shots.length - 1;
    $('#lightbox').hidden = false;
  }

  function closeLightbox() {
    $('#lightbox').hidden = true;
    $('#lb-img').src = '';
  }

  function stepLightbox(delta) {
    const next = state.shotIndex + delta;
    if (next < 0 || next >= state.shots.length) return;
    openLightbox(next);
  }

  $('#lb-close').onclick = closeLightbox;
  $('#lb-prev').onclick = () => stepLightbox(-1);
  $('#lb-next').onclick = () => stepLightbox(1);

  $('#lb-copy').onclick = async () => {
    const shot = state.shots[state.shotIndex];
    if (!shot) return;
    try {
      await api.screenshots.copy(state.profile.id, shot.name);
      toast(t('shots.copied'), 'ok');
    } catch (err) { toast(err.message, 'err'); }
  };

  $('#lb-reveal').onclick = () => {
    const shot = state.shots[state.shotIndex];
    if (shot) api.screenshots.reveal(state.profile.id, shot.name).catch((err) => toast(err.message, 'err'));
  };

  $('#lb-del').onclick = () => {
    const shot = state.shots[state.shotIndex];
    if (!shot) return;
    confirmDialog({
      title: t('btn.delete'),
      text: t('shots.deleteConfirm', { name: shot.name }),
      confirmLabel: t('btn.delete'),
      onConfirm: async () => {
        try {
          await api.screenshots.remove(state.profile.id, shot.name);
          closeLightbox();
          toast(t('shots.deleted'), 'ok');
          if (state.instanceTab === 'shots') renderShots();
          else renderInstanceOverview();
        } catch (err) { toast(err.message, 'err'); }
      },
    });
  };

  document.addEventListener('keydown', (e) => {
    if ($('#lightbox').hidden) return;
    if (e.key === 'Escape') closeLightbox();
    if (e.key === 'ArrowLeft') stepLightbox(-1);
    if (e.key === 'ArrowRight') stepLightbox(1);
  });

  // =========================================================================
  // logs
  //
  // What the game wrote, what it crashed with, and what the launcher captured,
  // with a one-click share through mclo.gs for fast support handoffs.
  // =========================================================================

  function renderInstanceLogs() {
    const profile = state.profile;
    if (!profile) return;
    const panel = $('#itab-logs');
    panel.innerHTML = `
      <div class="tab-toolbar">
        <button class="btn" data-act="refresh" data-icon="refresh"><span>${esc(t('servers.refresh'))}</span></button>
        <button class="btn" data-act="folder" data-icon="folder"><span>${esc(t('logs.openFolder'))}</span></button>
      </div>
      <div class="log-shell">
        <div class="log-list" id="log-list"></div>
        <div class="log-view">
          <pre class="log-out" id="log-out">${esc(t('logs.select'))}</pre>
          <div class="log-share" id="log-share" hidden></div>
        </div>
      </div>`;
    hydrateIcons(panel);

    $('[data-act="refresh"]', panel).onclick = () => renderInstanceLogs();
    $('[data-act="folder"]', panel).onclick = () => api.logs.openFolder(profile.id).catch((err) => toast(err.message, 'err'));

    loadLogList(profile.id);
  }

  async function loadLogList(profileId) {
    let list = [];
    try {
      list = await api.logs.list(profileId);
    } catch (err) {
      toast(err.message, 'err');
    }
    const box = $('#log-list');
    if (!box) return;
    state.logs = list;

    if (!list.length) {
      box.innerHTML = `<div class="empty"><div class="et">${esc(t('logs.none'))}</div>${esc(t('logs.noneHint'))}</div>`;
      return;
    }
    box.innerHTML = list.map((l) => `
      <button class="log-item" data-id="${esc(l.id)}">
        <div class="li-name">${esc(l.name)}</div>
        <div class="li-sub">${esc(t(`logs.kind.${l.kind}`))} · ${esc(fmtBytes(l.size))} · ${esc(fmtDateTime(l.modified))}</div>
      </button>`).join('');

    $$('.log-item', box).forEach((btn) => { btn.onclick = () => openLog(profileId, btn.dataset.id); });
    openLog(profileId, list[0].id); // the newest run is what a player came for
  }

  /** Colour the levels the way the console drawer does, line by line. */
  function logHtml(text) {
    const lines = text.split(/\r?\n/);
    // A full log can run to hundreds of thousands of lines; the tail is the
    // part that explains a crash, and the file itself stays on disk.
    const shown = lines.length > 4000 ? lines.slice(-4000) : lines;
    return shown.map((line) => {
      const cls = /\bERROR\b|Exception|Caused by:|^\s+at /.test(line) ? 'err'
        : /\bWARN\b/.test(line) ? 'warn' : '';
      return cls ? `<span class="${cls}">${esc(line)}</span>` : esc(line);
    }).join('\n');
  }

  async function openLog(profileId, id) {
    $$('#log-list .log-item').forEach((b) => b.classList.toggle('active', b.dataset.id === id));
    const out = $('#log-out');
    const share = $('#log-share');
    if (!out) return;
    out.textContent = '…';
    share.hidden = true;
    state.logId = id;

    let log;
    try {
      log = await api.logs.read(profileId, id);
    } catch (err) {
      out.textContent = err.message;
      return;
    }
    if (state.logId !== id || !$('#log-out')) return; // a different log was picked

    $('#log-out').innerHTML = logHtml(log.text);
    $('#log-out').scrollTop = $('#log-out').scrollHeight;

    share.hidden = false;
    share.innerHTML = `
      <button class="btn sm" data-act="upload" data-icon="upload"><span>${esc(t('logs.upload'))}</span></button>
      <span id="log-share-out">${log.truncated ? esc(t('logs.truncated', { size: fmtBytes(log.bytes) })) : ''}</span>`;
    hydrateIcons(share);

    $('[data-act="upload"]', share).onclick = async (e) => {
      const btn = e.currentTarget;
      btn.disabled = true;
      $('#log-share-out').textContent = t('logs.uploading');
      try {
        const res = await api.logs.upload(profileId, id);
        $('#log-share-out').innerHTML =
          `<a href="#" data-url="${esc(res.url)}">${esc(res.url)}</a>`;
        $('#log-share-out a').onclick = (ev) => { ev.preventDefault(); api.openExternal(res.url); };
        await navigator.clipboard.writeText(res.url).catch(() => {});
        toast(t('logs.uploaded'), 'ok');
      } catch (err) {
        $('#log-share-out').textContent = '';
        toast(err.message, 'err');
      } finally {
        btn.disabled = false;
      }
    };
  }

  // =========================================================================
  // statistics
  // =========================================================================

  async function renderStats() {
    const days = state.statsDays || 14;
    $$('#stats-range .seg').forEach((b) => b.classList.toggle('active', Number(b.dataset.days) === days));

    let data;
    try {
      data = await api.stats.summary(days);
    } catch (err) {
      toast(err.message, 'err');
      return;
    }
    state.stats = data;

    const tile = (glyph, key, value, sub) => `
      <div class="stat-tile">
        <div class="st-k"><span data-icon="${glyph}"></span>${esc(key)}</div>
        <div class="st-v">${esc(value)}</div>
        <div class="st-s">${esc(sub || '')}</div>
      </div>`;

    const longest = data.longest
      ? `${fmtDuration(data.longest.ms)}`
      : fmtDuration(0);

    $('#stat-tiles').innerHTML = [
      tile('clock', t('stats.total'), fmtDuration(data.totalMs), t('stats.totalSub')),
      tile('chart', t('stats.window', { n: days }), fmtDuration(data.windowMs), ''),
      tile('heart', t('stats.longest'), longest, data.longest?.name || ''),
      tile('layers', t('stats.mostPlayed'), data.mostPlayed?.name || '—',
        data.mostPlayed ? fmtDuration(data.mostPlayed.totalMs) : t('stats.noProfiles')),
    ].join('');
    hydrateIcons($('#stat-tiles'));

    $('#stat-chart-sum').textContent = t('stats.windowSum', { time: fmtDuration(data.windowMs) });

    const chart = $('#stat-chart');
    chart.classList.toggle('dense', data.days.length > 14);
    const peak = Math.max(...data.days.map((d) => d.ms), 1);
    const columns = data.days.map((d) => {
      const pct = Math.round((d.ms / peak) * 100);
      const label = d.date.slice(8) + '.' + d.date.slice(5, 7);
      const tooltip = `${label} · ${fmtChartDuration(d.ms)}`;
      return `<div class="chart-col ${d.ms ? 'has-play' : ''}" data-tip-text="${esc(tooltip)}"
                   tabindex="0" aria-label="${esc(tooltip)}">
        <div class="chart-bar" style="height:${d.ms ? Math.max(pct, 3) : 0}%"></div>
        <div class="chart-label">${esc(label)}</div>
      </div>`;
    }).join('');
    chart.innerHTML = `
      <div class="chart-y-axis" aria-hidden="true">
        <span>${esc(fmtChartDuration(peak))}</span>
        <span>${esc(fmtChartDuration(peak / 2))}</span>
        <span>0 min</span>
      </div>
      <div class="chart-plot">
        <div class="chart-grid-lines" aria-hidden="true"><i></i><i></i><i></i></div>
        <div class="chart-bars">${columns}</div>
      </div>`;

    const rows = $('#stat-profiles');
    if (!data.perProfile.length) {
      rows.innerHTML = `<div class="empty"><div class="et">${esc(t('stats.none'))}</div>${esc(t('stats.noneHint'))}</div>`;
      return;
    }
    const top = Math.max(...data.perProfile.map((p) => p.totalMs), 1);
    rows.innerHTML = data.perProfile.map((p) => {
      const profile = state.profiles.find((x) => x.id === p.id) || p;
      return `<div class="row" data-id="${esc(p.id)}">
        ${coverHtml(profile, 'sm')}
        <div class="row-meta">
          <div class="row-name">${esc(p.name)}</div>
          <div class="row-sub">${esc(p.mcVersion)} · ${esc(LOADER_LABEL[p.loader] || p.loader)} · ${esc(fmtDate(p.lastPlayed))}</div>
          <div class="bar-row" style="margin-top:6px">
            <div class="bar-track"><div class="bar-fill" style="width:${Math.round((p.totalMs / top) * 100)}%"></div></div>
          </div>
        </div>
        <span class="row-state">${esc(fmtDuration(p.totalMs))}</span>
      </div>`;
    }).join('');

    $$('.row', rows).forEach((row) => { row.onclick = () => openInstance(row.dataset.id); });
  }

  $$('#stats-range .seg').forEach((btn) => {
    btn.onclick = () => { state.statsDays = Number(btn.dataset.days); renderStats(); };
  });

  // =========================================================================
  // onboarding
  //
  // Shown once, on the very first start. Everything it asks has a working
  // default, so skipping it leaves a usable launcher behind.
  // =========================================================================

  const OB_STEPS = ['welcome', 'language', 'account', 'prefs', 'done'];
  const ob = { step: 0, language: 'de', ram: null };

  function startOnboarding() {
    ob.step = 0;
    ob.language = state.config.language || 'de';
    ob.ram = state.config.ram;
    $('#onboarding').hidden = false;
    renderOnboarding();
  }

  async function finishOnboarding() {
    $('#onboarding').hidden = true;
    state.config = await api.app.setConfig({
      firstRunDone: true,
      language: ob.language,
      ram: ob.ram || state.config.ram,
    });
    window.i18n.setLanguage(state.config.language);
    await refreshState();
  }

  function renderOnboarding() {
    const step = OB_STEPS[ob.step];
    $('#ob-steps').innerHTML = OB_STEPS
      .map((_, i) => `<span class="${i < ob.step ? 'done' : i === ob.step ? 'now' : ''}"></span>`).join('');

    const body = $('#ob-body');
    if (step === 'welcome') {
      body.innerHTML = `<span class="ob-mark">${logoMark(64)}</span>
        <h2>${esc(t('onboarding.welcomeTitle'))}</h2>
        <p>${esc(t('onboarding.welcomeText'))}</p>`;
    }

    if (step === 'language') {
      body.innerHTML = `<h2>${esc(t('onboarding.langTitle'))}</h2>
        <p>${esc(t('onboarding.langText'))}</p>
        <div class="ob-choice" id="ob-langs">
          ${[['de', 'Deutsch'], ['en', 'English']].map(([code, name]) => `
            <button class="${ob.language === code ? 'active' : ''}" data-lang="${code}">
              <span><span class="oc-t">${esc(name)}</span></span>
            </button>`).join('')}
        </div>`;
      $$('#ob-langs button', body).forEach((btn) => {
        btn.onclick = () => {
          ob.language = btn.dataset.lang;
          window.i18n.setLanguage(ob.language);
          renderOnboarding();
        };
      });
    }

    if (step === 'account') {
      const signed = state.account;
      body.innerHTML = `<h2>${esc(t('onboarding.accountTitle'))}</h2>
        <p>${esc(signed ? t('onboarding.accountDone', { name: signed.name }) : t('onboarding.accountText'))}</p>
        <div class="ob-choice">
          <button data-act="msa">
            <span data-icon="user"></span>
            <span><span class="oc-t">${esc(t('accounts.addMs'))}</span>
                  <span class="oc-d">${esc(t('accounts.sub'))}</span></span>
          </button>
          <button data-act="offline">
            <span data-icon="plus"></span>
            <span><span class="oc-t">${esc(t('accounts.addOffline'))}</span>
                  <span class="oc-d">${esc(t('accounts.offlineHint'))}</span></span>
          </button>
        </div>`;
      hydrateIcons(body);
      $('[data-act="msa"]', body).onclick = async () => {
        try {
          const account = await api.auth.signIn();
          if (account) { await refreshState(); renderOnboarding(); }
        } catch (err) { toast(err.message, 'err'); }
      };
      $('[data-act="offline"]', body).onclick = () => {
        // The overlay sits above the modal layer, so step aside while it asks.
        $('#onboarding').hidden = true;
        openOfflineDialog(async () => {
          $('#onboarding').hidden = false;
          renderOnboarding();
        });
      };
    }

    if (step === 'prefs') {
      const ram = ob.ram || state.config.ram || 4096;
      body.innerHTML = `<h2>${esc(t('onboarding.prefsTitle'))}</h2>
        <p>${esc(t('onboarding.prefsText'))}</p>
        <div class="field">
          <div class="range-head"><label>${esc(t('settings.ram'))}</label><b id="ob-ram-label">${esc(fmtGB(ram))}</b></div>
          <input type="range" id="ob-ram" min="1024" max="${state.totalRam}" step="512" value="${ram}" />
          <div class="hint">${esc(t('settings.ramHint', { total: fmtGB(state.totalRam) }))}</div>
        </div>`;
      const slider = $('#ob-ram', body);
      slider.oninput = () => {
        ob.ram = Number(slider.value);
        $('#ob-ram-label').textContent = fmtGB(slider.value);
      };
    }

    if (step === 'done') {
      body.innerHTML = `<span class="ob-mark">${logoMark(64)}</span>
        <h2>${esc(t('onboarding.doneTitle'))}</h2>
        <p>${esc(t('onboarding.doneText'))}</p>`;
    }

    $('#ob-back').hidden = ob.step === 0;
    $('#ob-skip').hidden = ob.step === OB_STEPS.length - 1;
    $('#ob-next').textContent = ob.step === OB_STEPS.length - 1 ? t('onboarding.finish') : t('onboarding.next');
  }

  $('#ob-next').onclick = () => {
    if (ob.step === OB_STEPS.length - 1) { finishOnboarding(); return; }
    ob.step++;
    renderOnboarding();
  };
  $('#ob-back').onclick = () => { if (ob.step > 0) { ob.step--; renderOnboarding(); } };
  $('#ob-skip').onclick = () => finishOnboarding();

  // =========================================================================
  // console
  // =========================================================================

  function pushConsole(line, level = 'info') {
    const cls = level === 'error' ? 'err' : /WARN/i.test(line) ? 'warn' : /ERROR|Exception/i.test(line) ? 'err' : '';
    const bytes = line.length * 2;
    state.consoleLines.push({ line, cls, bytes });
    consoleBytes += bytes;

    const out = $('#console-out');
    const span = document.createElement('span');
    span.className = cls;
    span.textContent = `${line}\n`;
    out.appendChild(span);
    while (state.consoleLines.length > CONSOLE_LIMIT || consoleBytes > CONSOLE_BYTE_LIMIT) {
      const removed = state.consoleLines.shift();
      consoleBytes -= removed?.bytes || 0;
      out.removeChild(out.firstChild);
    }
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
      setLaunchProgressVisible(false);
      viewer?.setWalking(true);
    } else if (s === 'stopped' || s === 'crashed') {
      state.running.delete(profileId);
      setLaunchProgressVisible(false);
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
    if ($('#view-instance').classList.contains('active')) renderInstance();
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
    for (const [profileId, updates] of state.modUpdates) applyModUpdates(updates, profileId);
    // Covers are tiny local 256px files. Warm every profile once so the rail,
    // switcher and profile grid never depend on which instance was opened first.
    await ensureCovers(state.profiles.map((profile) => profile.id), { rerender: false });
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
    if ($('#view-instance').classList.contains('active')) renderInstance();
    if ($('#view-stats').classList.contains('active')) renderStats();
    renderServerList();
    syncViewer();
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
    await loadDefaultSkin();
    if (state.account) loadSkin(state.account.uuid);

    pingServers();

    // The main process watches every instance's mods folder, so a jar dropped
    // in while the launcher is open appears without a click.
    api.on.modsChanged(({ profileId, mods }) => applyMods(profileId, mods));

    renderAll();

    // Network housekeeping starts after the shell is interactive. Browser
    // results warm first; the more expensive installed-content update scan
    // still runs once per launch, just without competing with initial paint.
    setTimeout(warmContentSearch, 450);
    setTimeout(checkModUpdatesOnStartup, 1800);

    // Last, so the overlay opens over a launcher that is already up to date.
    if (!state.config.firstRunDone) startOnboarding();
  }

  init().catch((err) => {
    console.error(err);
    document.body.innerHTML = `<div style="padding:40px;color:#e08a82;font-family:ui-monospace,monospace">Startup failed: ${esc(err.message)}</div>`;
  });
})();
