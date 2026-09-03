'use strict';
const fs = require('fs');
const os = require('os');
const path = require('path');
const crypto = require('crypto');
const { safeStorage } = require('electron');
const P = require('./paths');
const { configPatch, profilePatch } = require('./validation');
const { UUID_RE } = require('./validation');
const { encryptionAvailable, serializeAccount, deserializeAccount, hasPlaintextCredentials } = require('./credentials');
const { containsPath } = require('./security');
const { readJSON, writeJSONAtomic } = require('./persistence');

function totalRamMB() {
  return Math.floor(os.totalmem() / 1024 / 1024);
}

/** Suggest a heap size: half of RAM, clamped to something sane. */
function defaultRamMB() {
  const half = Math.floor(totalRamMB() / 2 / 512) * 512;
  return Math.max(2048, Math.min(8192, half));
}

const DEFAULT_CONFIG = {
  language: 'de',
  theme: 'mango',
  ram: defaultRamMB(),
  javaPath: '',           // empty = auto-provision
  javaArgs: '',
  fullscreen: false,
  width: 1280,
  height: 720,
  keepLauncherOpen: true,
  hideOnLaunch: false,
  showSnapshots: false,
  mangoConfig: true,      // load MangoConfig into every instance on launch
  performanceMods: true,  // put Sodium/Lithium/FerriteCore into Fabric instances
  sidebarOpen: true,
  concurrentDownloads: 12,
  performancePreset: 'balanced', // potato | balanced | quality
  selectedProfile: null,
  selectedAccount: null,
  firstRunDone: false,
  telemetry: true,        // count this copy on the website (anonymous id only)
};

function externalizeInlineIcons(profiles) {
  let changed = false;
  const dir = path.join(P.cache, 'mod-icons');
  for (const profile of profiles) {
    for (const mod of (Array.isArray(profile?.mods) ? profile.mods : [])) {
      const match = /^data:image\/png;base64,([A-Za-z0-9+/=]+)$/.exec(mod.icon || '');
      if (!match) continue;
      try {
        const data = Buffer.from(match[1], 'base64');
        const hash = crypto.createHash('sha1').update(data).digest('hex');
        const file = path.join(dir, `${hash}.png`);
        fs.mkdirSync(dir, { recursive: true });
        if (!fs.existsSync(file)) fs.writeFileSync(file, data);
        mod.icon = `mangoimg://f/${Buffer.from(file, 'utf8').toString('base64url')}`;
        changed = true;
      } catch { /* keep malformed legacy data untouched */ }
    }
  }
  return changed;
}

function safeMods(profileId, mods) {
  const base = path.resolve(P.instanceDir(profileId));
  const allowed = new Set(['mods', 'shaderpacks', 'resourcepacks', 'datapacks']);
  return mods.map((mod) => {
    if (mod.filename && path.basename(mod.filename) !== mod.filename) throw new Error('Invalid mod filename');
    if (!mod.file) return mod;
    const target = path.resolve(mod.file);
    const rel = path.relative(base, target);
    if (!containsPath(base, target) || !allowed.has(rel.split(path.sep)[0])) throw new Error('Mod file is outside its profile');
    try {
      const stat = fs.lstatSync(target);
      if (stat.isSymbolicLink()) throw new Error('Mod file cannot be a symlink');
      const real = fs.realpathSync(target);
      if (!containsPath(base, real)) throw new Error('Mod file escapes its profile');
    } catch (err) {
      if (err.code !== 'ENOENT') throw err;
    }
    return { ...mod, file: target };
  });
}

function storedProfile(raw) {
  if (!raw || typeof raw !== 'object' || !UUID_RE.test(raw.id || '')) return null;
  const defaults = {
    name: 'New Profile', mcVersion: '1.21.11', loader: 'vanilla', loaderVersion: '', color: null,
    cover: false, ram: null, javaArgs: '', mods: [], mangoConfig: null, session: null,
    lastPlayed: null, playTimeMs: 0,
  };
  const clean = { id: raw.id, ...defaults };
  for (const key of Object.keys(defaults)) {
    if (!Object.prototype.hasOwnProperty.call(raw, key)) continue;
    try { clean[key] = profilePatch({ [key]: raw[key] })[key]; } catch { /* retain default */ }
  }
  // One stale or unsafe legacy mod path must not discard the profile that owns
  // the player's worlds. Keep every valid record and let folder sync rediscover
  // local jars after startup.
  clean.mods = clean.mods.flatMap((mod) => {
    try { return safeMods(clean.id, [mod]); }
    catch (err) {
      console.warn(`[profiles] Ignoring unsafe content in ${clean.id}: ${err.message}`);
      return [];
    }
  });
  // The picture file is the source of truth. This repairs older metadata that
  // lost `cover: true` while the actual image remained safely on disk.
  clean.cover = fs.existsSync(P.coverFile(clean.id));
  clean.created = Number.isSafeInteger(raw.created) && raw.created >= 0 ? raw.created : Date.now();
  return clean;
}

class Store {
  constructor() {
    P.ensureDirs();
    const savedConfig = readJSON(P.config, {});
    let cleanConfig = {};
    for (const key of Object.keys(DEFAULT_CONFIG)) {
      if (!Object.prototype.hasOwnProperty.call(savedConfig, key)) continue;
      try { Object.assign(cleanConfig, configPatch({ [key]: savedConfig[key] })); } catch { /* use the safe default */ }
    }
    this.config = { ...DEFAULT_CONFIG, ...cleanConfig };
    const acc = readJSON(P.accounts, { accounts: [], profiles: [] });
    const rawProfiles = Array.isArray(acc.profiles) ? acc.profiles : [];
    // Older releases stored local mod icons as large inline data URLs. Migrate
    // those before validation, otherwise one legacy icon can reset the whole
    // stored mod list and discard its Modrinth metadata.
    const migratedInlineIcons = externalizeInlineIcons(rawProfiles);
    this.accounts = (Array.isArray(acc.accounts) ? acc.accounts : []).flatMap((account) => {
      try {
        const restored = deserializeAccount(account, safeStorage);
        if (restored.credentialsLocked) console.warn('[credentials] Encrypted account tokens are locked until the OS keyring is available or the account signs in again');
        return [restored];
      } catch (err) {
        console.warn(`[credentials] ${err.message}`);
        return [];
      }
    });
    this.profiles = rawProfiles.flatMap((profile) => {
      try {
        const clean = storedProfile(profile);
        return clean ? [clean] : [];
      } catch (err) {
        console.warn(`[profiles] Ignoring unsafe profile ${profile?.id || ''}: ${err.message}`);
        return [];
      }
    });
    const plaintext = this.accounts.some(hasPlaintextCredentials);
    if (plaintext && !encryptionAvailable(safeStorage)) {
      console.warn('[credentials] OS key storage is unavailable; account tokens remain protected by file mode 0600 only');
    }
    if (migratedInlineIcons) this.saveAccounts();
    // Rewriting migrates legacy plaintext credentials to safeStorage whenever
    // the operating system offers a real credential backend.
    if (plaintext && encryptionAvailable(safeStorage)) this.saveAccounts();
    if (this.profiles.length === 0) this.createDefaultProfile();
  }

  createDefaultProfile() {
    const profile = this.addProfile({
      name: 'MangoClient 1.21.11',
      mcVersion: '1.21.11',
      loader: 'fabric',
    });
    this.config.selectedProfile = profile.id;
    this.saveConfig();
  }

  saveConfig() {
    writeJSONAtomic(P.config, this.config);
  }

  saveAccounts() {
    writeJSONAtomic(P.accounts, {
      accounts: this.accounts.map((account) => serializeAccount(account, safeStorage)),
      profiles: this.profiles,
    });
  }

  setConfig(patch) {
    Object.assign(this.config, configPatch(patch));
    this.saveConfig();
    return this.config;
  }

  // ---- profiles -----------------------------------------------------------

  addProfile(data) {
    const { id: _id, created: _created, ...candidate } = data || {};
    const clean = profilePatch(candidate);
    const profile = {
      id: crypto.randomUUID(),
      name: clean.name || 'New Profile',
      mcVersion: clean.mcVersion || '1.21.11',
      loader: clean.loader || 'vanilla',      // vanilla | fabric | quilt | neoforge
      loaderVersion: clean.loaderVersion || '',
      color: clean.color || null,              // null = derived from the id
      cover: clean.cover || false,             // true = covers/<id>.png exists
      ram: clean.ram || null,                  // null = inherit global
      javaArgs: clean.javaArgs || '',
      mods: clean.mods || [],                  // installed Modrinth mods
      mangoConfig: clean.mangoConfig ?? null,  // null = follow the global setting
      session: clean.session || null,          // {pid} while the game is playing
      lastPlayed: clean.lastPlayed || null,
      playTimeMs: clean.playTimeMs || 0,
      created: Date.now(),
    };
    this.profiles.push(profile);
    this.saveAccounts();
    fs.mkdirSync(path.join(P.instanceDir(profile.id), 'mods'), { recursive: true });
    return profile;
  }

  updateProfile(id, patch) {
    const p = this.profiles.find((x) => x.id === id);
    if (!p) return null;
    const clean = profilePatch(patch);
    if (clean.mods) clean.mods = safeMods(id, clean.mods);
    Object.assign(p, clean);
    this.saveAccounts();
    return p;
  }

  deleteProfile(id) {
    this.profiles = this.profiles.filter((p) => p.id !== id);
    if (this.config.selectedProfile === id) {
      this.config.selectedProfile = this.profiles[0]?.id || null;
      this.saveConfig();
    }
    this.saveAccounts();
    try {
      fs.rmSync(P.instanceDir(id), { recursive: true, force: true });
      fs.rmSync(P.coverFile(id), { force: true });
    } catch { /* instance dir may not exist yet */ }
  }

  getProfile(id) {
    return this.profiles.find((p) => p.id === id) || null;
  }

  get selectedProfile() {
    return this.getProfile(this.config.selectedProfile) || this.profiles[0] || null;
  }

  // ---- accounts -----------------------------------------------------------

  upsertAccount(account) {
    const idx = this.accounts.findIndex((a) => a.uuid === account.uuid);
    if (idx >= 0) this.accounts[idx] = { ...this.accounts[idx], ...account };
    else this.accounts.push(account);
    if (!this.config.selectedAccount) {
      this.config.selectedAccount = account.uuid;
      this.saveConfig();
    }
    this.saveAccounts();
    return account;
  }

  removeAccount(uuid) {
    this.accounts = this.accounts.filter((a) => a.uuid !== uuid);
    if (this.config.selectedAccount === uuid) {
      this.config.selectedAccount = this.accounts[0]?.uuid || null;
      this.saveConfig();
    }
    this.saveAccounts();
  }

  get selectedAccount() {
    return this.accounts.find((a) => a.uuid === this.config.selectedAccount) || this.accounts[0] || null;
  }
}

module.exports = { Store, totalRamMB, defaultRamMB, DEFAULT_CONFIG };
