'use strict';
const fs = require('fs');
const os = require('os');
const path = require('path');
const crypto = require('crypto');
const P = require('./paths');

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
  sidebarOpen: true,
  concurrentDownloads: 12,
  performancePreset: 'balanced', // potato | balanced | quality
  selectedProfile: null,
  selectedAccount: null,
  firstRunDone: false,
};

function readJSON(file, fallback) {
  try {
    return JSON.parse(fs.readFileSync(file, 'utf8'));
  } catch {
    return fallback;
  }
}

function writeJSONAtomic(file, data) {
  fs.mkdirSync(path.dirname(file), { recursive: true });
  const tmp = `${file}.tmp`;
  fs.writeFileSync(tmp, JSON.stringify(data, null, 2));
  fs.renameSync(tmp, file);
}

class Store {
  constructor() {
    P.ensureDirs();
    this.config = { ...DEFAULT_CONFIG, ...readJSON(P.config, {}) };
    const acc = readJSON(P.accounts, { accounts: [], profiles: [] });
    this.accounts = acc.accounts || [];
    this.profiles = acc.profiles || [];
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
    writeJSONAtomic(P.accounts, { accounts: this.accounts, profiles: this.profiles });
  }

  setConfig(patch) {
    Object.assign(this.config, patch);
    this.saveConfig();
    return this.config;
  }

  // ---- profiles -----------------------------------------------------------

  addProfile(data) {
    const profile = {
      id: crypto.randomUUID(),
      name: data.name || 'New Profile',
      mcVersion: data.mcVersion || '1.21.11',
      loader: data.loader || 'vanilla',      // vanilla | fabric | quilt | neoforge
      loaderVersion: data.loaderVersion || '',
      color: data.color || null,              // null = derived from the id
      cover: data.cover || false,             // true = covers/<id>.png exists
      ram: data.ram || null,                  // null = inherit global
      javaArgs: data.javaArgs || '',
      mods: data.mods || [],                  // installed Modrinth mods
      session: null,                          // {pid} while the game is playing
      lastPlayed: null,
      playTimeMs: 0,
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
    Object.assign(p, patch);
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
