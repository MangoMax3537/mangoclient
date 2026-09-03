'use strict';
const path = require('path');

// Chromium and GTK can otherwise share the host's global Fontconfig cache even
// when their bundled Fontconfig revisions differ. Give this process an
// equivalent config with an app-local cache, while respecting an explicit user
// override.
if (process.platform === 'linux' && !process.env.FONTCONFIG_FILE) {
  const packaged = __dirname.includes('app.asar');
  process.env.FONTCONFIG_FILE = packaged
    ? path.join(process.resourcesPath, 'app.asar.unpacked', 'src', 'assets', 'fontconfig.conf')
    : path.join(__dirname, '..', 'assets', 'fontconfig.conf');
}

const { app, BrowserWindow, ipcMain, shell, dialog, nativeImage, protocol, net, session } = require('electron');
const fs = require('fs');
const fsp = require('fs/promises');
const { pathToFileURL } = require('url');

const P = require('./paths');
const { Store, totalRamMB } = require('./store');
const {
  isExactRendererUrl, isAllowedExternalUrl, isTrustedIpcSender, approvedImagePath,
} = require('./security');

const RENDERER_FILE = path.join(__dirname, '..', 'renderer', 'index.html');

function lazyModule(request) {
  let loaded = null;
  return new Proxy(Object.create(null), {
    get(_target, property) {
      loaded ||= require(request);
      return loaded[property];
    },
  });
}

const auth = lazyModule('./auth');
const installer = lazyModule('./installer');
const loaders = lazyModule('./loaders');
const javaMod = lazyModule('./java');
const modrinth = lazyModule('./modrinth');
const localmods = lazyModule('./localmods');
const servers = lazyModule('./servers');
const skins = lazyModule('./skins');
const mangoconfig = lazyModule('./mangoconfig');
const performancemods = lazyModule('./performancemods');
const screenshots = lazyModule('./screenshots');
const gamelogs = lazyModule('./gamelogs');
const stats = lazyModule('./stats');
const telemetry = lazyModule('./telemetry');
const storage = lazyModule('./storage');
const launcher = lazyModule('./launcher');

app.setName('MangoClient');

function appendFeatureSwitch(name, features) {
  const current = app.commandLine.getSwitchValue(name)
    .split(',')
    .map((feature) => feature.trim())
    .filter(Boolean);
  app.commandLine.appendSwitch(name, [...new Set([...current, ...features])].join(','));
}

// Chromium cannot present Vulkan surfaces through native Wayland, and some
// compositors advertise color-management support that cannot describe sRGB.
// Both paths already fall back after logging errors, so select the fallback up
// front. X11/XWayland keeps Chromium's defaults.
const waylandSession = process.platform === 'linux'
  && Boolean(process.env.WAYLAND_DISPLAY);
const useXwayland = waylandSession
  && Boolean(process.env.DISPLAY)
  && process.env.MANGO_USE_XWAYLAND === '1';
if (useXwayland) {
  app.commandLine.removeSwitch('ozone-platform');
  app.commandLine.appendSwitch('ozone-platform', 'x11');
} else if (waylandSession) {
  appendFeatureSwitch('disable-features', ['WaylandWpColorManagerV1']);
}

// Screenshots sit in the instance folder, which the renderer's CSP will not
// load over file://. A scheme of our own serves them - and nothing else.
protocol.registerSchemesAsPrivileged([
  { scheme: 'mangoimg', privileges: { standard: true, secure: true, supportFetchAPI: true, stream: true } },
]);
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

function createUpdaterController(onState) {
  const disabledReason = !app.isPackaged
    ? 'dev'
    : process.platform === 'linux' && !process.env.APPIMAGE ? 'unsupported' : null;
  if (!disabledReason) return require('./updater').createUpdater({ onState });

  const state = { state: 'disabled', reason: disabledReason, current: app.getVersion() };
  return {
    get state() { return state; },
    async check() { onState(state); return state; },
    install() { return false; },
    startPeriodicChecks() { return this; },
  };
}

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
    if (await launcher.isGameAlive(pid)) detached.set(profile.id, pid);
    else rememberSession(profile.id, null);
  }
  watchDetachedSessions();
}

/** Adopted sessions have no child handle, so their end has to be noticed. */
function watchDetachedSessions() {
  if (detachedTimer || detached.size === 0) return;
  detachedTimer = setInterval(async () => {
    for (const [profileId, pid] of [...detached]) {
      if (await launcher.isGameAlive(pid)) continue;
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

/** Pick up jars added while the launcher was closed before the UI reads state. */
function syncSelectedMods() {
  const profile = store.selectedProfile;
  if (!profile) return;
  try { syncMods(profile.id); } catch (err) { console.error('[mods:sync]', err); }
}

/** Persist the dependency graph for old profiles before an operation relies
 * on it. New installs already carry this metadata. */
async function prepareProfileDependencies(profile) {
  const hydrated = await modrinth.hydrateDependencyMetadata(profile);
  if (hydrated.changed) store.updateProfile(profile.id, { mods: hydrated.mods });
  return { profile: store.getProfile(profile.id), complete: hydrated.complete };
}

/**
 * Watch an instance's mods folder so a jar dropped in while the launcher is
 * open shows up without the player having to click anything.
 */
function watchMods(profileId) {
  if (modWatchers.has(profileId)) return;
  const dir = path.join(P.instanceDir(profileId), 'mods');
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

/** Watch only profiles whose folder can affect visible or running state. */
function refreshModWatchers() {
  const ids = new Set([
    store.selectedProfile?.id,
    ...running.keys(),
    ...detached.keys(),
  ].filter(Boolean));
  for (const id of [...modWatchers.keys()]) if (!ids.has(id)) closeModWatcher(id);
  for (const id of ids) watchMods(id);
}

/** Serve exactly the pictures inside an instance folder, and refuse the rest. */
function registerImageProtocol() {
  protocol.handle('mangoimg', async (request) => {
    const url = new URL(request.url);
    const coverId = url.hostname === 'cover'
      ? decodeURIComponent(url.pathname.replace(/^\//, ''))
      : null;
    const isCover = Boolean(coverId && /^[a-f0-9-]+$/i.test(coverId) && store.getProfile(coverId)?.cover);
    const candidate = isCover ? P.coverFile(coverId) : screenshots.fileFromUrl(request.url);
    const roots = [
      P.covers,
      path.join(P.cache, 'mod-icons'),
      ...store.profiles.map((profile) => path.join(P.instanceDir(profile.id), 'screenshots')),
    ];
    const file = await approvedImagePath(candidate, roots);
    if (!file) {
      return new Response('Not found', { status: 404 });
    }
    return net.fetch(pathToFileURL(file).toString());
  });
}

function createWindow() {
  win = new BrowserWindow({
    width: 1280,
    height: 800,
    minWidth: 1040,
    minHeight: 680,
    show: false,
    backgroundColor: '#191614',
    frame: false,
    titleBarStyle: 'hidden',
    icon: path.join(__dirname, '..', 'assets', 'icon.png'),
    webPreferences: {
      preload: path.join(__dirname, 'preload.js'),
      contextIsolation: true,
      nodeIntegration: false,
      sandbox: true,
      spellcheck: false,
    },
  });

  win.loadFile(RENDERER_FILE);
  win.once('ready-to-show', () => win.show());

  // External links belong in the user's browser, never in the launcher shell.
  win.webContents.setWindowOpenHandler(({ url }) => {
    if (isAllowedExternalUrl(url)) shell.openExternal(url);
    return { action: 'deny' };
  });
  win.webContents.on('will-navigate', (event, url) => {
    if (isExactRendererUrl(url, RENDERER_FILE)) return;
    event.preventDefault();
    if (isAllowedExternalUrl(url)) shell.openExternal(url);
  });

  if (process.argv.includes('--dev')) win.webContents.openDevTools({ mode: 'detach' });
}

// ---------------------------------------------------------------------------
// IPC
// ---------------------------------------------------------------------------

/** Wrap a handler so renderer-side callers always get {ok, data|error}. */
function handle(channel, fn) {
  ipcMain.handle(channel, async (event, ...args) => {
    try {
      if (!trustedIpcSender(event)) throw new Error('Unauthorized IPC sender');
      return { ok: true, data: await fn(...args) };
    } catch (err) {
      console.error(`[ipc:${channel}]`, err);
      return { ok: false, error: err.message || String(err), needsRelogin: Boolean(err.needsRelogin) };
    }
  });
}

function trustedIpcSender(event) {
  return Boolean(win && !win.isDestroyed()
    && isTrustedIpcSender(event, win.webContents, RENDERER_FILE));
}

function onTrusted(channel, fn) {
  ipcMain.on(channel, (event, ...args) => {
    if (!trustedIpcSender(event)) return;
    fn(...args);
  });
}

function registerIpc() {
  // --- window controls
  onTrusted('window:minimize', () => win?.minimize());
  onTrusted('window:maximize', () => (win?.isMaximized() ? win.unmaximize() : win?.maximize()));
  onTrusted('window:close', () => win?.close());
  onTrusted('open:external', (url) => {
    if (isAllowedExternalUrl(url)) shell.openExternal(url);
  });

  // --- bootstrap
  handle('app:state', async () => {
    syncSelectedMods();
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
      : P.instanceDir(requireProfile(which).id);
    await fsp.mkdir(target, { recursive: true });
    shell.openPath(target);
    return target;
  });
  handle('app:openContentFolder', async (profileId, type) => {
    const folders = { mod: 'mods', resourcepack: 'resourcepacks', shader: 'shaderpacks' };
    const folder = folders[type];
    if (!folder) throw new Error('Unknown content type');
    const profile = requireProfile(profileId);
    const target = path.join(P.instanceDir(profile.id), folder);
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
  handle('profile:select', async (id) => {
    const config = store.setConfig({ selectedProfile: id });
    syncSelectedMods();
    refreshModWatchers();
    return config;
  });
  // --- profile pictures
  /** Let Chromium stream and cache a cover instead of copying it as base64. */
  function coverUrl(id, version = '') {
    const file = P.coverFile(id);
    if (!fs.existsSync(file)) return null;
    const suffix = version ? `?v=${encodeURIComponent(version)}` : '';
    return `mangoimg://cover/${encodeURIComponent(id)}${suffix}`;
  }

  handle('profile:covers', async () =>
    Object.fromEntries(store.profiles
      .map((p) => [p.id, p.cover ? coverUrl(p.id) : null])
      .filter(([, url]) => url)));

  handle('profile:cover', async (id) => {
    const profile = store.getProfile(id);
    if (!profile?.cover) return null;
    return coverUrl(id);
  });

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
    const cover = P.coverFile(id);
    const temp = `${cover}.${process.pid}-${Date.now()}.tmp`;
    try {
      await fsp.writeFile(temp, resized.toPNG(), { flag: 'wx', mode: 0o600 });
      await fsp.rename(temp, cover);
    } catch (err) {
      await fsp.unlink(temp).catch(() => {});
      throw err;
    }
    store.updateProfile(id, { cover: true });
    return coverUrl(id, Date.now());
  });

  handle('profile:clearCover', async (id) => {
    await fsp.rm(P.coverFile(id), { force: true });
    store.updateProfile(id, { cover: false });
    return true;
  });

  handle('profile:duplicate', async (id) => {
    const src = store.getProfile(id);
    if (!src) throw new Error('Profile not found');
    const copy = store.addProfile({ ...src, name: `${src.name} (copy)`, cover: false, mods: [] });
    try {
      // Copy the instance folder so mods/configs/worlds come along. A failed
      // copy is reported and rolled back instead of leaving an empty profile
      // that looks like a successful duplicate.
      const sourceDir = P.instanceDir(id);
      const copyDir = P.instanceDir(copy.id);
      await fsp.cp(sourceDir, copyDir, { recursive: true });
      if (src.cover && fs.existsSync(P.coverFile(id))) {
        await fsp.copyFile(P.coverFile(id), P.coverFile(copy.id));
      }
      const mods = (src.mods || []).map((mod) => ({
        ...mod,
        file: mod.file ? path.join(copyDir, path.relative(sourceDir, mod.file)) : mod.file,
      }));
      store.updateProfile(copy.id, { cover: src.cover && fs.existsSync(P.coverFile(copy.id)), mods });
      refreshModWatchers();
      return store.getProfile(copy.id);
    } catch (err) {
      store.deleteProfile(copy.id);
      throw new Error(`Instance could not be duplicated: ${err.message}`);
    }
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
    let profile = store.getProfile(profileId);
    if (!profile) throw new Error('Profile not found');
    syncMods(profileId);
    profile = store.getProfile(profileId);
    const account = store.selectedAccount;
    if (!account) throw new Error('Add a Minecraft account first.');

    // A vanilla profile cannot load mods, so any profile MangoConfig has a
    // build for is quietly moved onto Fabric first - otherwise the game would
    // start without it every time. Forge, NeoForge and Quilt profiles are
    // left exactly as they are: switching those would break their own mods.
    if ((!profile.loader || profile.loader === 'vanilla')
      && mangoconfig.enabledFor(profile, store.config)
      && mangoconfig.hasBuildFor(profile.mcVersion)) {
      store.updateProfile(profileId, { loader: 'fabric', loaderVersion: '' });
      profile = store.getProfile(profileId);
      send('launch:log', {
        profileId,
        line: `${mangoconfig.NAME} needs a mod loader: this profile now uses Fabric`,
        level: 'info',
      });
    }

    // MangoConfig rides along on every launch, so the in-game HUD is there
    // whether or not the player thought about it. It never throws: starting
    // without it beats refusing to start.
    const [mc] = await Promise.all([
      mangoconfig.ensure({
        profile,
        config: store.config,
        onLog: (line) => send('launch:log', { profileId, line, level: 'info' }),
      }),
      // These helpers touch separate jars and can be prepared together.
      performancemods.ensure({
        profile,
        config: store.config,
        onLog: (line) => send('launch:log', { profileId, line, level: 'info' }),
      }),
    ]);
    if (mc.error) {
      send('launch:log', { profileId, line: `${mangoconfig.NAME} skipped: ${mc.error}`, level: 'warn' });
    } else if (mc.state === 'unsupported') {
      send('launch:log', {
        profileId,
        line: `${mangoconfig.NAME} has no build for ${mc.reason}, starting without it`,
        level: 'warn',
      });
    }

    const instance = await launcher.launch({
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
    refreshModWatchers();
    // Recorded now, because the launcher may well be closed before the game is.
    rememberSession(profileId, { pid: instance.pid, versionId: instance.versionId, startedAt: Date.now() });
    instance.on('exit', () => {
      running.delete(profileId);
      refreshModWatchers();
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
    let profile = store.getProfile(profileId);
    if (!profile) throw new Error('Profile not found');
    ({ profile } = await prepareProfileDependencies(profile));
    const dependents = modrinth.installedDependents(profile, projectId);
    if (dependents.length) {
      return {
        blocked: true,
        dependents: dependents.map((record) => ({ projectId: record.projectId, title: record.title })),
        mods: profile.mods || [],
      };
    }
    await modrinth.uninstallMod(profile, projectId);
    const mods = (profile.mods || []).filter((m) => m.projectId !== projectId);
    store.updateProfile(profileId, { mods });
    return { blocked: false, dependents: [], mods: store.getProfile(profileId).mods };
  });

  handle('modrinth:toggle', async (profileId, projectId, enabled) => {
    let profile = store.getProfile(profileId);
    if (!profile) throw new Error('Profile not found');
    const prepared = await prepareProfileDependencies(profile);
    profile = prepared.profile;
    if (!prepared.complete) throw new Error('Mod dependencies could not be verified. Check your internet connection and try again.');
    const result = await modrinth.setEnabledWithDependencies(profile, projectId, enabled);
    store.updateProfile(profileId, { mods: result.mods });
    return { mods: store.getProfile(profileId).mods, affected: result.affected };
  });

  handle('modrinth:checkUpdates', async (profileId) => {
    let profile = store.getProfile(profileId);
    if (!profile) throw new Error('Profile not found');
    ({ profile } = await prepareProfileDependencies(profile));
    return modrinth.checkUpdates(profile);
  });

  handle('modrinth:checkAllUpdates', async () => {
    const checked = [];
    for (const stored of store.profiles) {
      try {
        const { profile } = await prepareProfileDependencies(stored);
        checked.push({ profileId: profile.id, updates: await modrinth.checkUpdates(profile) });
      } catch (err) {
        checked.push({ profileId: stored.id, updates: [], error: err.message || String(err) });
      }
    }
    return checked;
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

  // --- MangoConfig
  handle('mangoconfig:info', async (profileId) => {
    const profile = profileId ? store.getProfile(profileId) : store.selectedProfile;
    return {
      name: mangoconfig.NAME,
      loaders: mangoconfig.LOADERS,
      gameVersions: mangoconfig.GAME_VERSIONS,
      globalEnabled: store.config.mangoConfig !== false,
      enabled: profile ? mangoconfig.enabledFor(profile, store.config) : false,
      supported: profile ? mangoconfig.supports(profile) : false,
      installed: profile ? mangoconfig.present(profile.id) : false,
    };
  });

  // --- screenshots
  handle('shots:list', async (profileId) => {
    requireProfile(profileId);
    return screenshots.listScreenshots(profileId);
  });
  handle('shots:delete', async (profileId, name) => {
    requireProfile(profileId);
    return screenshots.deleteScreenshot(profileId, name);
  });
  handle('shots:reveal', async (profileId, name) => {
    requireProfile(profileId);
    return screenshots.revealScreenshot(profileId, name);
  });
  handle('shots:copy', async (profileId, name) => {
    requireProfile(profileId);
    return screenshots.copyScreenshot(profileId, name);
  });
  handle('shots:folder', async (profileId) => {
    requireProfile(profileId);
    return screenshots.openFolder(profileId);
  });

  // --- logs
  handle('logs:list', async (profileId) => gamelogs.listLogs(profileId, launcherLogFor(profileId)));
  handle('logs:read', async (profileId, id) => gamelogs.readLog(profileId, id, launcherLogFor(profileId)));
  handle('logs:upload', async (profileId, id) => gamelogs.uploadLog(profileId, id, launcherLogFor(profileId)));
  handle('logs:delete', async (profileId, id) => gamelogs.deleteLog(profileId, id, launcherLogFor(profileId)));
  handle('logs:folder', async (profileId) => {
    requireProfile(profileId);
    return gamelogs.openLogsFolder(profileId);
  });

  // --- statistics
  handle('stats:summary', async (days) => stats.summary(store.profiles, days || 14));

  // --- storage
  handle('storage:usage', async () => storage.usage());
  handle('storage:clear', async (key) => {
    if (runningProfileIds().length) throw new Error('Close the game before clearing files');
    return storage.clear(key);
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
      backgroundColor: '#191614',
      webPreferences: {
        // An in-memory partition: no launcher cookies leak in, none linger after.
        partition: 'msa-login',
        nodeIntegration: false,
        contextIsolation: true,
        sandbox: true,
      },
    });
    authWin.webContents.setWindowOpenHandler(() => ({ action: 'deny' }));
    authWin.webContents.session.setPermissionRequestHandler((_wc, _permission, callback) => callback(false));
    authWin.webContents.session.setPermissionCheckHandler(() => false);

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

function requireProfile(id) {
  const profile = store.getProfile(id);
  if (!profile) throw new Error('Profile not found');
  return profile;
}

/** The launcher's own capture of a profile's last run, shown beside the game's. */
function launcherLogFor(profileId) {
  return launcher.logFileFor(requireProfile(profileId));
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
    session.defaultSession.setPermissionRequestHandler((_wc, _permission, callback) => callback(false));
    session.defaultSession.setPermissionCheckHandler(() => false);
    store = new Store();
    updater = createUpdaterController((s) => send('update:state', s));
    registerImageProtocol();
    registerIpc();
    refreshModWatchers();
    // What the website counts: an anonymous id per copy, beaten while we run.
    telemetry.start(() => store.config.telemetry !== false);
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
