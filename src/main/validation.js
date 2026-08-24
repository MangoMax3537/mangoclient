'use strict';

const UUID_RE = /^(?:[0-9a-f]{32}|[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12})$/i;
const VERSION_RE = /^[0-9A-Za-z][0-9A-Za-z._+-]{0,63}$/;
const COLOR_RE = /^#[0-9a-f]{6}$/i;
const LOADERS = new Set(['vanilla', 'fabric', 'quilt', 'forge', 'neoforge']);

function object(value, label = 'value') {
  if (!value || typeof value !== 'object' || Array.isArray(value)) throw new Error(`${label} must be an object`);
  return value;
}
function bool(v, name) { if (typeof v !== 'boolean') throw new Error(`${name} must be boolean`); return v; }
function text(v, name, max, allowEmpty = true) {
  if (typeof v !== 'string' || v.length > max || (!allowEmpty && !v.trim())) throw new Error(`Invalid ${name}`);
  if (v.includes('\0')) throw new Error(`Invalid ${name}`);
  return v;
}
function integer(v, name, min, max) {
  if (!Number.isInteger(v) || v < min || v > max) throw new Error(`Invalid ${name}`);
  return v;
}
function nullableId(v, name) {
  if (v === null) return null;
  if (typeof v !== 'string' || !UUID_RE.test(v)) throw new Error(`Invalid ${name}`);
  return v;
}
function enumValue(v, name, values) {
  if (!values.has(v)) throw new Error(`Invalid ${name}`);
  return v;
}

const CONFIG_FIELDS = {
  language: (v) => enumValue(v, 'language', new Set(['de', 'en'])),
  theme: (v) => enumValue(v, 'theme', new Set(['mango'])),
  ram: (v) => integer(v, 'ram', 1024, 131072),
  javaPath: (v) => text(v, 'javaPath', 4096),
  javaArgs: (v) => text(v, 'javaArgs', 8192),
  fullscreen: (v) => bool(v, 'fullscreen'),
  width: (v) => integer(v, 'width', 640, 16384),
  height: (v) => integer(v, 'height', 480, 16384),
  keepLauncherOpen: (v) => bool(v, 'keepLauncherOpen'),
  hideOnLaunch: (v) => bool(v, 'hideOnLaunch'),
  showSnapshots: (v) => bool(v, 'showSnapshots'),
  mangoConfig: (v) => bool(v, 'mangoConfig'),
  performanceMods: (v) => bool(v, 'performanceMods'),
  sidebarOpen: (v) => bool(v, 'sidebarOpen'),
  concurrentDownloads: (v) => integer(v, 'concurrentDownloads', 1, 32),
  performancePreset: (v) => enumValue(v, 'performancePreset', new Set(['potato', 'balanced', 'quality'])),
  selectedProfile: (v) => nullableId(v, 'selectedProfile'),
  selectedAccount: (v) => nullableId(v, 'selectedAccount'),
  firstRunDone: (v) => bool(v, 'firstRunDone'),
  telemetry: (v) => bool(v, 'telemetry'),
};

function filterPatch(patch, fields, label) {
  object(patch, label);
  const clean = {};
  for (const [key, value] of Object.entries(patch)) {
    const validate = fields[key];
    if (!validate) throw new Error(`Unknown ${label} setting: ${key}`);
    clean[key] = validate(value);
  }
  return clean;
}

function configPatch(patch) { return filterPatch(patch, CONFIG_FIELDS, 'configuration'); }

function validateMods(v) {
  if (!Array.isArray(v) || v.length > 10000) throw new Error('Invalid mods');
  return v.map((mod) => {
    object(mod, 'mod');
    const clean = {};
    const strings = ['projectId', 'slug', 'title', 'versionId', 'versionNumber', 'filename', 'file', 'icon'];
    for (const key of strings) if (mod[key] != null) clean[key] = text(mod[key], key, key === 'file' || key === 'icon' ? 8192 : 512);
    if (mod.type != null) clean.type = enumValue(mod.type, 'mod type', new Set(['mod', 'shader', 'resourcepack', 'datapack']));
    for (const key of ['enabled', 'dependency', 'local']) if (mod[key] != null) clean[key] = bool(mod[key], key);
    if (mod.installedAt != null) clean.installedAt = integer(mod.installedAt, 'installedAt', 0, Number.MAX_SAFE_INTEGER);
    if (Array.isArray(mod.gameVersions)) clean.gameVersions = mod.gameVersions.slice(0, 128).map((x) => text(x, 'gameVersion', 64));
    if (Array.isArray(mod.loaders)) clean.loaders = mod.loaders.slice(0, 16).map((x) => text(x, 'loader', 32));
    return clean;
  });
}

const PROFILE_FIELDS = {
  name: (v) => text(v, 'profile name', 128, false).trim(),
  mcVersion: (v) => { v = text(v, 'Minecraft version', 64, false); if (!VERSION_RE.test(v)) throw new Error('Invalid Minecraft version'); return v; },
  loader: (v) => enumValue(v, 'loader', LOADERS),
  loaderVersion: (v) => { v = text(v, 'loader version', 64); if (v && !VERSION_RE.test(v)) throw new Error('Invalid loader version'); return v; },
  color: (v) => { if (v === null) return null; if (typeof v !== 'string' || !COLOR_RE.test(v)) throw new Error('Invalid color'); return v.toLowerCase(); },
  cover: (v) => bool(v, 'cover'),
  ram: (v) => v === null ? null : integer(v, 'profile ram', 1024, 131072),
  javaArgs: (v) => text(v, 'profile javaArgs', 8192),
  mods: validateMods,
  mangoConfig: (v) => v === null ? null : bool(v, 'mangoConfig'),
  session: (v) => {
    if (v === null) return null;
    object(v, 'session');
    return { pid: integer(v.pid, 'pid', 1, 0x7fffffff), versionId: text(v.versionId || '', 'versionId', 128), startedAt: integer(v.startedAt || 0, 'startedAt', 0, Number.MAX_SAFE_INTEGER) };
  },
  lastPlayed: (v) => v === null ? null : integer(v, 'lastPlayed', 0, Number.MAX_SAFE_INTEGER),
  playTimeMs: (v) => integer(v, 'playTimeMs', 0, Number.MAX_SAFE_INTEGER),
};

function profilePatch(patch) {
  if (Object.prototype.hasOwnProperty.call(object(patch, 'profile'), 'id')) throw new Error('Profile IDs are immutable');
  return filterPatch(patch, PROFILE_FIELDS, 'profile');
}

module.exports = { UUID_RE, VERSION_RE, COLOR_RE, LOADERS, configPatch, profilePatch, validateMods };
