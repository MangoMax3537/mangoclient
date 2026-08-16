'use strict';
const { app, BrowserWindow, ipcMain, shell, dialog, nativeImage } = require('electron');
const path = require('path');
const fs = require('fs');
const fsp = require('fs/promises');

const P = require('./paths');
const { Store, totalRamMB } = require('./store');
const auth = require('./auth');
const installer = require('./installer');
const loaders = require('./loaders');
const javaMod = require('./java');
const modrinth = require('./modrinth');
const localmods = require('./localmods');
const servers = require('./servers');
const skins = require('./skins');
const { launch, isGameAlive } = require('./launcher');
const { createUpdater } = require('./updater');

app.setName('MangoClient');
// Minecraft's own launcher does this too; some Linux/GPU combos need it.
app.commandLine.appendSwitch('enable-features', 'WaylandWindowDecorations');

let store;
let win = null;
let updater = null;
/** profileId -> GameInstance */
const running = new Map();
/**
 * Games still playing from an earlier launcher run: profileId -> pid. Closing
 * the launcher leaves the game alone, so on the next start the session has to
 * be recognised again - if only to keep a second JVM off the same world.
 */
const detached = new Map();
let detachedTimer = null;

function send(channel, payload) {
  if (win && !win.isDestroyed()) win.webContents.send(channel, payload);
}

/** Everything the UI should show a Stop button for. */
function runningProfileIds() {
  return [...new Set([...running.keys(), ...detached.keys()])];
}

/** Note (or forget) the session that survives us, so the next start knows. */
function rememberSession(profileId, session) {
  store.updateProfile(profileId, { session });
}

/** Adopt sessions recorded before the last shutdown that are still playing. */
async function adoptDetachedSessions() {
  for (const profile of store.profiles) {
    const pid = profile.session?.pid;
    if (!pid) continue;
    if (await isGameAlive(pid)) detached.set(profile.id, pid);
    else rememberSession(profile.id, null);
  }
  watchDetachedSessions();
}

/** Adopted sessions have no child handle, so their end has to be noticed. */
function watchDetachedSessions() {
  if (detachedTimer || detached.size === 0) return;
  detachedTimer = setInterval(async () => {
    for (const [profileId, pid] of [...detached]) {
      if (await isGameAlive(pid)) continue;
      detached.delete(profileId);
      rememberSession(profileId, null);
      send('launch:state', { profileId, state: 'stopped' });
    }
    if (detached.size === 0) {
      clearInterval(detachedTimer);
      detachedTimer = null;
    }
  }, 10000);
  detachedTimer.unref?.();
}

// ---------------------------------------------------------------------------
// Manually added mods
//
// Jars copied into an instance's `mods` folder are loaded by the game either
// way, so the list has to follow the folder rather than only its own records.
// ---------------------------------------------------------------------------

/** profileId -> {watcher, timer} */
const modWatchers = new Map();

/** Fold the mods folder into a profile's list. Returns the current list. */
function syncMods(profileId) {
  const profile = store.getProfile(profileId);
  if (!profile) return [];
  const { mods, changed } = localmods.syncProfileMods(profile);
  if (changed) store.updateProfile(profileId, { mods });
  return { mods, changed };
}

function syncAllMods() {
  for (const profile of store.profiles) {
    try { syncMods(profile.id); } catch (err) { console.error('[mods:sync]', err); }
  }
}

/**
 * Watch an instance's mods folder so a jar dropped in while the launcher is
 * open shows up without the player having to click anything.
 */
function watchMods(profileId) {
  if (modWatchers.has(profileId)) return;
  const dir = localmods.modsDirFor(profileId);
  try {
    fs.mkdirSync(dir, { recursive: true });
    const entry = { timer: null, watcher: null };
    entry.watcher = fs.watch(dir, () => {
      // Copying a large jar fires several events; settle before reading it.
      clearTimeout(entry.timer);
      entry.timer = setTimeout(() => {
        try {
          const { mods, changed } = syncMods(profileId);
          if (changed) send('mods:changed', { profileId, mods });
        } catch (err) { console.error('[mods:watch]', err); }
      }, 500);
    });
    entry.watcher.on('error', () => closeModWatcher(profileId));
    modWatchers.set(profileId, entry);
  } catch (err) {
    console.error('[mods:watch]', err); // fall back to syncing on demand
  }
}

function closeModWatcher(profileId) {
  const entry = modWatchers.get(profileId);
  if (!entry) return;
  clearTimeout(entry.timer);
  try { entry.watcher?.close(); } catch { /* already gone */ }
  modWatchers.delete(profileId);
}

/** Keep one watcher per existing profile, and none for deleted ones. */
function refreshModWatchers() {
  const ids = new Set(store.profiles.map((p) => p.id));
  for (const id of [...modWatchers.keys()]) if (!ids.has(id)) closeModWatcher(id);
  for (const id of ids) watchMods(id);
}

function createWindow() {
  win = new BrowserWindow({
    width: 1280,
    height: 800,
    minWidth: 1040,
    minHeight: 680,
    show: false,
    backgroundColor: '#131316',
    frame: false,
    titleBarStyle: 'hidden',
    icon: path.join(__dirname, '..', 'renderer', 'assets', 'icon.png'),
    webPreferences: {
      preload: path.join(__dirname, 'preload.js'),
      contextIsolation: true,
      nodeIntegration: false,
      sandbox: false,
      spellcheck: false,
    },
  });

  win.loadFile(path.join(__dirname, '..', 'renderer', 'index.html'));
  win.once('ready-to-show', () => win.show());

  // External links belong in the user's browser, never in the launcher shell.
  win.webContents.setWindowOpenHandler(({ url }) => {
    shell.openExternal(url);
    return { action: 'deny' };
  });
  win.webContents.on('will-navigate', (event, url) => {
    if (!url.startsWith('file://')) {
      event.preventDefault();
      shell.openExternal(url);
    }
  });

  if (process.argv.includes('--dev')) win.webContents.openDevTools({ mode: 'detach' });
}

// ---------------------------------------------------------------------------
// IPC
// ---------------------------------------------------------------------------

/** Wrap a handler so renderer-side callers always get {ok, data|error}. */
function handle(channel, fn) {
  ipcMain.handle(channel, async (_event, ...args) => {
    try {
      return { ok: true, data: await fn(...args) };
    } catch (err) {
      console.error(`[ipc:${channel}]`, err);
      return { ok: false, error: err.message || String(err), needsRelogin: Boolean(err.needsRelogin) };
    }
  });
}

function registerIpc() {
  // --- window controls
  ipcMain.on('window:minimize', () => win?.minimize());
  ipcMain.on('window:maximize', () => (win?.isMaximized() ? win.unmaximize() : win?.maximize()));
  ipcMain.on('window:close', () => win?.close());
  ipcMain.on('open:external', (_e, url) => {
    if (/^https?:\/\//.test(url)) shell.openExternal(url);
  });

  // --- bootstrap
  handle('app:state', async () => {
    // Pick up jars that were added or removed while the launcher was closed.
    syncAllMods();
    return {
      config: store.config,
      profiles: store.profiles,
      accounts: store.accounts.map(publicAccount),
      selectedProfile: store.selectedProfile,
      selectedAccount: store.selectedAccount ? publicAccount(store.selectedAccount) : null,
      totalRam: totalRamMB(),
      paths: { root: P.root, instances: P.instances },
      version: app.getVersion(),
      running: runningProfileIds(),
    };
  });

  handle('config:set', async (patch) => store.setConfig(patch));

  // --- updates
  handle('update:state', async () => updater.state);
  handle('update:check', async () => updater.check());
  handle('update:install', async () => updater.install());
  handle('app:openFolder', async (which) => {
    const target = which === 'root' ? P.root
      : which === 'logs' ? P.logs
      : P.instanceDir(which);
    await fsp.mkdir(target, { recursive: true });
    shell.openPath(target);
    return target;
  });

  // --- accounts
  handle('auth:signIn', async () => {
    let code;
    try {
      code = await openSignInWindow();
    } catch (err) {
      if (err.cancelled) return null; // the player closed the window, not an error
      throw err;
    }
    const tokens = await auth.redeemCode(code);
    const account = await auth.loginWithLiveTokens(tokens);
    store.upsertAccount(account);
    warmSkin(account);
    return publicAccount(account);
  });

  handle('auth:addOffline', async (name) => {
    const account = auth.offlineAccount(name);
    store.upsertAccount(account);
    warmSkin(account);
    return publicAccount(account);
  });

  handle('auth:remove', async (uuid) => { store.removeAccount(uuid); return true; });
  handle('auth:select', async (uuid) => store.setConfig({ selectedAccount: uuid }));
  handle('auth:refresh', async (uuid) => {
    const account = store.accounts.find((a) => a.uuid === uuid);
    if (!account) throw new Error('Account not found');
    const fresh = await auth.ensureFreshAccount(account);
    store.upsertAccount(fresh);
    return publicAccount(fresh);
  });

  // Lets the start page show a player model before any account is added.
  handle('skin:default', async () => ({ skin: skins.defaultSkinDataUrl(), cape: null, slim: false, source: 'default' }));

  handle('skin:get', async (uuid) => {
    const account = store.accounts.find((a) => a.uuid === uuid);
    if (!account) throw new Error('Account not found');
    const cached = await skins.getCachedSkin(uuid);
    if (cached) {
      // Serve the cache immediately, refresh in the background.
      skins.getSkin(account).then((fresh) => send('skin:updated', { uuid, ...fresh })).catch(() => {});
      return cached;
    }
    return skins.getSkin(account);
  });

  handle('skin:upload', async (uuid, variant) => {
    const account = store.accounts.find((a) => a.uuid === uuid);
    if (!account) throw new Error('Account not found');
    const res = await dialog.showOpenDialog(win, {
      title: 'Choose a skin (64x64 PNG)',
      filters: [{ name: 'PNG image', extensions: ['png'] }],
      properties: ['openFile'],
    });
    if (res.canceled || !res.filePaths[0]) return null;
    await skins.uploadSkin(account, res.filePaths[0], variant || 'classic');
    const fresh = await auth.ensureFreshAccount({ ...account, expiresAt: 0 });
    store.upsertAccount(fresh);
    return skins.getSkin(fresh);
  });

  // --- profiles
  handle('profile:list', async () => store.profiles);
  handle('profile:create', async (data) => {
    const profile = store.addProfile(data);
    refreshModWatchers();
    return profile;
  });
  handle('profile:update', async (id, patch) => store.updateProfile(id, patch));
  handle('profile:delete', async (id) => {
    closeModWatcher(id);
    store.deleteProfile(id);
    return store.profiles;
  });
  handle('profile:select', async (id) => store.setConfig({ selectedProfile: id }));
  // --- profile pictures
  /** Read a profile's picture back as a data URL the renderer can show. */
  function coverDataUrl(id) {
    const file = P.coverFile(id);
    if (!fs.existsSync(file)) return null;
    const image = nativeImage.createFromPath(file);
    return image.isEmpty() ? null : image.toDataURL();
  }

  handle('profile:covers', async () =>
    Object.fromEntries(store.profiles
      .map((p) => [p.id, p.cover ? coverDataUrl(p.id) : null])
      .filter(([, url]) => url)));

  handle('profile:pickCover', async (id) => {
    if (!store.getProfile(id)) throw new Error('Profile not found');
    const res = await dialog.showOpenDialog(win, {
      title: 'Choose a picture for this profile',
      filters: [{ name: 'Images', extensions: ['png', 'jpg', 'jpeg', 'webp', 'gif', 'bmp'] }],
      properties: ['openFile'],
    });
    if (res.canceled || !res.filePaths[0]) return null;

    const source = nativeImage.createFromPath(res.filePaths[0]);
    if (source.isEmpty()) throw new Error('That file is not a readable image.');

    // Square it off around the centre, then store one modest-sized PNG.
    const { width, height } = source.getSize();
    const side = Math.min(width, height);
    const square = source.crop({
      x: Math.round((width - side) / 2),
      y: Math.round((height - side) / 2),
      width: side,
      height: side,
    });
    const resized = square.resize({ width: 256, height: 256, quality: 'best' });

    await fsp.mkdir(P.covers, { recursive: true });
    await fsp.writeFile(P.coverFile(id), resized.toPNG());
    store.updateProfile(id, { cover: true });
    return resized.toDataURL();
  });

  handle('profile:clearCover', async (id) => {
    await fsp.rm(P.coverFile(id), { force: true });
    store.updateProfile(id, { cover: false });
    return true;
  });

  handle('profile:duplicate', async (id) => {
    const src = store.getProfile(id);
    if (!src) throw new Error('Profile not found');
    const copy = store.addProfile({ ...src, name: `${src.name} (copy)`, mods: [] });
    refreshModWatchers();
    // Copy the instance folder so mods/configs/worlds come along.
    await fsp.cp(P.instanceDir(id), P.instanceDir(copy.id), { recursive: true }).catch(() => {});
    if (src.cover) await fsp.copyFile(P.coverFile(id), P.coverFile(copy.id)).catch(() => {});
    store.updateProfile(copy.id, { mods: (src.mods || []).map((m) => ({ ...m, file: m.file.replace(id, copy.id) })) });
    return store.getProfile(copy.id);
  });

  // --- versions & loaders
  handle('versions:manifest', async (force) => {
    const manifest = await installer.getVersionManifest(force);
    const installed = await installer.listInstalledVersions();
    return {
      latest: manifest.latest,
      versions: manifest.versions.map((v) => ({ id: v.id, type: v.type, releaseTime: v.releaseTime })),
      installed,
    };
  });
  handle('loaders:versions', async (loader, mcVersion) => loaders.listLoaderVersions(loader, mcVersion));
  handle('java:list', async () => {
    const [system, managed] = await Promise.all([javaMod.findSystemJavas(), javaMod.listManagedRuntimes()]);
    return { system, managed };
  });
  handle('java:pick', async () => {
    const res = await dialog.showOpenDialog(win, { title: 'Select a Java executable', properties: ['openFile'] });
    if (res.canceled || !res.filePaths[0]) return null;
    const info = await javaMod.probeJava(res.filePaths[0]);
    if (!info) throw new Error('That file is not a runnable Java executable.');
    return info;
  });
  handle('java:install', async (major) => {
    const exe = await javaMod.downloadRuntime(
      major,
      (p) => send('launch:progress', p),
      (line) => send('launch:log', { line, level: 'info' }),
    );
    return javaMod.probeJava(exe);
  });

  // --- launching
  handle('game:launch', async (profileId, quickJoinServer) => {
    if (running.has(profileId)) throw new Error('This profile is already running.');
    if (detached.has(profileId)) {
      // Two JVMs on one instance folder fight over the same world files.
      throw new Error('This profile is still running from before the launcher was restarted. Close the game first.');
    }
    const profile = store.getProfile(profileId);
    if (!profile) throw new Error('Profile not found');
    const account = store.selectedAccount;
    if (!account) throw new Error('Add a Minecraft account first.');

    const instance = await launch({
      profile,
      account,
      config: store.config,
      store,
      quickJoinServer: quickJoinServer || null,
      onProgress: (p) => send('launch:progress', { profileId, ...p }),
      onLog: (line, level = 'info') => send('launch:log', { profileId, line, level }),
      onState: (state) => send('launch:state', { profileId, state }),
    });

    running.set(profileId, instance);
    // Recorded now, because the launcher may well be closed before the game is.
    rememberSession(profileId, { pid: instance.pid, versionId: instance.versionId, startedAt: Date.now() });
    instance.on('exit', () => {
      running.delete(profileId);
      rememberSession(profileId, null);
      send('launch:state', { profileId, state: 'stopped' });
      if (store.config.hideOnLaunch && win && !win.isVisible()) win.show();
    });
    instance.on('running', () => {
      if (store.config.hideOnLaunch) win?.hide();
      else if (!store.config.keepLauncherOpen) win?.minimize();
    });

    return { pid: instance.pid, versionId: instance.versionId };
  });

  handle('game:stop', async (profileId) => {
    const instance = running.get(profileId);
    if (instance) {
      instance.kill();
      return true;
    }
    // A session adopted from an earlier run is only known by its pid.
    const pid = detached.get(profileId);
    if (!pid) return false;
    try { process.kill(pid); } catch { /* it beat us to it */ }
    detached.delete(profileId);
    rememberSession(profileId, null);
    send('launch:state', { profileId, state: 'stopped' });
    return true;
  });

  handle('game:running', async () => runningProfileIds());

  // --- mods on disk
  handle('mods:sync', async (profileId) => {
    if (!store.getProfile(profileId)) throw new Error('Profile not found');
    return syncMods(profileId).mods;
  });

  // --- Modrinth
  handle('modrinth:search', async (opts) => modrinth.search(opts));
  handle('modrinth:project', async (id) => modrinth.getProject(id));
  handle('modrinth:versions', async (id, opts) => modrinth.getVersions(id, opts));
  handle('modrinth:categories', async () => modrinth.getCategories());

  handle('modrinth:install', async (profileId, projectIdOrSlug, versionId) => {
    const profile = store.getProfile(profileId);
    if (!profile) throw new Error('Profile not found');
    const installed = await modrinth.installProject({
      profile,
      projectIdOrSlug,
      versionId,
      onLog: (line) => send('launch:log', { profileId, line, level: 'info' }),
    });
    const mods = [...(profile.mods || [])];
    for (const rec of installed) {
      const idx = mods.findIndex((m) => m.projectId === rec.projectId);
      if (idx >= 0) {
        // Replacing an existing mod, so drop the old jar.
        if (mods[idx].file !== rec.file) await fsp.unlink(mods[idx].file).catch(() => {});
        mods[idx] = { ...rec, enabled: mods[idx].enabled !== false };
      } else {
        mods.push({ ...rec, enabled: true });
      }
    }
    store.updateProfile(profileId, { mods });
    return installed;
  });

  handle('modrinth:uninstall', async (profileId, projectId) => {
    const profile = store.getProfile(profileId);
    if (!profile) throw new Error('Profile not found');
    await modrinth.uninstallMod(profile, projectId);
    store.updateProfile(profileId, { mods: (profile.mods || []).filter((m) => m.projectId !== projectId) });
    return store.getProfile(profileId).mods;
  });

  handle('modrinth:toggle', async (profileId, projectId, enabled) => {
    const profile = store.getProfile(profileId);
    if (!profile) throw new Error('Profile not found');
    const newPath = await modrinth.setModEnabled(profile, projectId, enabled);
    const mods = (profile.mods || []).map((m) =>
      m.projectId === projectId ? { ...m, enabled, file: newPath || m.file } : m);
    store.updateProfile(profileId, { mods });
    return mods;
  });

  handle('modrinth:checkUpdates', async (profileId) => {
    const profile = store.getProfile(profileId);
    if (!profile) throw new Error('Profile not found');
    return modrinth.checkUpdates(profile);
  });

  handle('modrinth:installModpack', async (profileId, versionId) => {
    const profile = store.getProfile(profileId);
    if (!profile) throw new Error('Profile not found');
    const info = await modrinth.installModpack({
      profile,
      versionId,
      onLog: (line) => send('launch:log', { profileId, line, level: 'info' }),
      onProgress: (p) => send('launch:progress', { profileId, ...p }),
    });
    store.updateProfile(profileId, {
      mcVersion: info.mcVersion || profile.mcVersion,
      loader: info.loader || profile.loader,
      loaderVersion: info.loaderVersion || '',
      name: info.name || profile.name,
    });
    return info;
  });

  // --- servers
  handle('servers:partners', async () => servers.FEATURED_SERVERS);
  handle('servers:ping', async (address) => servers.pingServer(address));
  handle('servers:pingAll', async (list) => {
    const targets = list || servers.FEATURED_SERVERS;
    // Stream each result so the sidebar fills in as pings land.
    servers.pingAll(targets, (result) => send('servers:pinged', result));
    return targets;
  });

}

/**
 * Show Microsoft's login pages in a modal window and resolve with the OAuth
 * code they redirect back with. Rejects with `.cancelled` if the player closes
 * the window instead of finishing.
 */
function openSignInWindow() {
  return new Promise((resolve, reject) => {
    const authWin = new BrowserWindow({
      width: 520,
      height: 720,
      parent: win || undefined,
      modal: Boolean(win),
      show: false,
      autoHideMenuBar: true,
      title: 'Microsoft',
      backgroundColor: '#131316',
      webPreferences: {
        // An in-memory partition: no launcher cookies leak in, none linger after.
        partition: 'msa-login',
        nodeIntegration: false,
        contextIsolation: true,
        sandbox: true,
      },
    });

    let settled = false;
    const finish = (fn, value) => {
      if (settled) return;
      settled = true;
      fn(value);
      if (!authWin.isDestroyed()) authWin.destroy();
    };

    const inspect = (url) => {
      const result = auth.readRedirect(url);
      if (!result) return;
      if (result.cancelled) finish(reject, Object.assign(new Error('cancelled'), { cancelled: true }));
      else if (result.error) finish(reject, new Error(result.error));
      else finish(resolve, result.code);
    };

    authWin.webContents.on('will-redirect', (_e, url) => inspect(url));
    authWin.webContents.on('will-navigate', (_e, url) => inspect(url));
    authWin.webContents.on('did-navigate', (_e, url) => inspect(url));
    authWin.webContents.on('did-fail-load', (_e, errorCode, errorDescription, url) => {
      // The redirect target itself never resolves; that's expected, not a failure.
      if (url && url.startsWith(auth.REDIRECT_URI)) return;
      if (errorCode === -3) return; // ERR_ABORTED, fired on every internal redirect
      finish(reject, new Error(`Could not reach Microsoft (${errorDescription})`));
    });

    authWin.on('closed', () => {
      if (settled) return;
      settled = true;
      reject(Object.assign(new Error('cancelled'), { cancelled: true }));
    });

    authWin.once('ready-to-show', () => authWin.show());
    authWin.loadURL(auth.authorizeUrl());
  });
}

/** Never hand tokens to the renderer; it only needs identity and cosmetics. */
function publicAccount(account) {
  return {
    uuid: account.uuid,
    name: account.name,
    type: account.type,
    expiresAt: account.expiresAt,
    expired: account.type !== 'offline' && account.expiresAt ? account.expiresAt < Date.now() : false,
    hasCape: (account.capes || []).some((c) => c.state === 'ACTIVE'),
  };
}

function warmSkin(account) {
  skins.getSkin(account)
    .then((skin) => send('skin:updated', { uuid: account.uuid, ...skin }))
    .catch(() => {});
}

// ---------------------------------------------------------------------------

const gotLock = app.requestSingleInstanceLock();
if (!gotLock) {
  app.quit();
} else {
  app.on('second-instance', () => {
    if (win) { if (win.isMinimized()) win.restore(); win.focus(); }
  });

  app.whenReady().then(() => {
    P.ensureDirs();
    store = new Store();
    updater = createUpdater({ onState: (s) => send('update:state', s) });
    registerIpc();
    syncAllMods();
    refreshModWatchers();
    adoptDetachedSessions().catch((err) => console.error('[sessions]', err));
    createWindow();

    // The launch check installs and relaunches by itself, so a player always
    // starts on the current version. Later checks only offer the update.
    setTimeout(() => updater.check({ apply: true }).then(() => updater.startPeriodicChecks()), 1500);

    app.on('activate', () => {
      if (BrowserWindow.getAllWindows().length === 0) createWindow();
    });
  });

  app.on('window-all-closed', () => {
    if (process.platform !== 'darwin') app.quit();
  });

  app.on('before-quit', () => {
    // A running game keeps playing without us. Record the session now, since
    // the child's 'close' event will never reach this process.
    for (const instance of running.values()) instance.flushPlayTime();
    for (const id of [...modWatchers.keys()]) closeModWatcher(id);
  });
}
