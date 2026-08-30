'use strict';
const { contextBridge, ipcRenderer } = require('electron');

/**
 * Thin, explicit surface for the renderer. Every invoke resolves to
 * {ok, data} / {ok:false, error}; `call` unwraps it into a plain
 * value-or-throw so UI code reads naturally.
 */
async function call(channel, ...args) {
  const res = await ipcRenderer.invoke(channel, ...args);
  if (!res?.ok) {
    const err = new Error(res?.error || 'Unknown error');
    err.needsRelogin = res?.needsRelogin;
    throw err;
  }
  return res.data;
}

const listeners = new Map();
function on(channel, cb) {
  const wrapped = (_e, payload) => cb(payload);
  ipcRenderer.on(channel, wrapped);
  listeners.set(cb, wrapped);
  return () => {
    ipcRenderer.off(channel, wrapped);
    listeners.delete(cb);
  };
}

contextBridge.exposeInMainWorld('mango', {
  // window chrome
  window: {
    minimize: () => ipcRenderer.send('window:minimize'),
    maximize: () => ipcRenderer.send('window:maximize'),
    close: () => ipcRenderer.send('window:close'),
  },
  openExternal: (url) => ipcRenderer.send('open:external', url),

  app: {
    state: () => call('app:state'),
    setConfig: (patch) => call('config:set', patch),
    openFolder: (which) => call('app:openFolder', which),
    openContentFolder: (profileId, type) => call('app:openContentFolder', profileId, type),
  },

  update: {
    state: () => call('update:state'),
    check: () => call('update:check'),
    install: () => call('update:install'),
  },

  auth: {
    signIn: () => call('auth:signIn'),
    addOffline: (name) => call('auth:addOffline', name),
    remove: (uuid) => call('auth:remove', uuid),
    select: (uuid) => call('auth:select', uuid),
    refresh: (uuid) => call('auth:refresh', uuid),
  },

  skins: {
    get: (uuid) => call('skin:get', uuid),
    default: () => call('skin:default'),
    upload: (uuid, variant) => call('skin:upload', uuid, variant),
  },

  profiles: {
    list: () => call('profile:list'),
    create: (data) => call('profile:create', data),
    update: (id, patch) => call('profile:update', id, patch),
    remove: (id) => call('profile:delete', id),
    select: (id) => call('profile:select', id),
    duplicate: (id) => call('profile:duplicate', id),
    covers: () => call('profile:covers'),
    cover: (id) => call('profile:cover', id),
    pickCover: (id) => call('profile:pickCover', id),
    clearCover: (id) => call('profile:clearCover', id),
  },

  versions: {
    manifest: (force) => call('versions:manifest', force),
    loaderVersions: (loader, mc) => call('loaders:versions', loader, mc),
  },

  java: {
    list: () => call('java:list'),
    pick: () => call('java:pick'),
    install: (major) => call('java:install', major),
  },

  game: {
    launch: (profileId, quickJoin) => call('game:launch', profileId, quickJoin),
    stop: (profileId) => call('game:stop', profileId),
    running: () => call('game:running'),
  },

  mods: {
    sync: (profileId) => call('mods:sync', profileId),
  },

  modrinth: {
    search: (opts) => call('modrinth:search', opts),
    project: (id) => call('modrinth:project', id),
    versions: (id, opts) => call('modrinth:versions', id, opts),
    categories: () => call('modrinth:categories'),
    install: (profileId, project, versionId) => call('modrinth:install', profileId, project, versionId),
    uninstall: (profileId, projectId) => call('modrinth:uninstall', profileId, projectId),
    toggle: (profileId, projectId, enabled) => call('modrinth:toggle', profileId, projectId, enabled),
    checkUpdates: (profileId) => call('modrinth:checkUpdates', profileId),
    checkAllUpdates: () => call('modrinth:checkAllUpdates'),
    installModpack: (profileId, versionId) => call('modrinth:installModpack', profileId, versionId),
  },

  mangoConfig: {
    info: (profileId) => call('mangoconfig:info', profileId),
  },

  screenshots: {
    list: (profileId) => call('shots:list', profileId),
    remove: (profileId, name) => call('shots:delete', profileId, name),
    reveal: (profileId, name) => call('shots:reveal', profileId, name),
    copy: (profileId, name) => call('shots:copy', profileId, name),
    openFolder: (profileId) => call('shots:folder', profileId),
  },

  logs: {
    list: (profileId) => call('logs:list', profileId),
    read: (profileId, id) => call('logs:read', profileId, id),
    upload: (profileId, id) => call('logs:upload', profileId, id),
    remove: (profileId, id) => call('logs:delete', profileId, id),
    openFolder: (profileId) => call('logs:folder', profileId),
  },

  stats: {
    summary: (days) => call('stats:summary', days),
  },

  storage: {
    usage: () => call('storage:usage'),
    clear: (key) => call('storage:clear', key),
  },

  servers: {
    partners: () => call('servers:partners'),
    ping: (address) => call('servers:ping', address),
    pingAll: (list) => call('servers:pingAll', list),
  },

  on: {
    launchProgress: (cb) => on('launch:progress', cb),
    launchLog: (cb) => on('launch:log', cb),
    launchState: (cb) => on('launch:state', cb),
    skinUpdated: (cb) => on('skin:updated', cb),
    updateState: (cb) => on('update:state', cb),
    serverPinged: (cb) => on('servers:pinged', cb),
    modsChanged: (cb) => on('mods:changed', cb),
  },
});
