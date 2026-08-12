'use strict';
const os = require('os');
const path = require('path');
const fs = require('fs');

/**
 * MangoClient keeps everything in one self-contained root so that uninstalling
 * is a single `rm -rf`, and so multiple profiles can share libraries/assets.
 */
function rootDir() {
  if (process.env.MANGOCLIENT_HOME) return process.env.MANGOCLIENT_HOME;
  if (process.platform === 'win32') {
    return path.join(process.env.APPDATA || path.join(os.homedir(), 'AppData', 'Roaming'), '.mangoclient');
  }
  if (process.platform === 'darwin') {
    return path.join(os.homedir(), 'Library', 'Application Support', 'mangoclient');
  }
  return path.join(os.homedir(), '.mangoclient');
}

const ROOT = rootDir();

const P = {
  root: ROOT,
  versions: path.join(ROOT, 'versions'),
  libraries: path.join(ROOT, 'libraries'),
  assets: path.join(ROOT, 'assets'),
  assetIndexes: path.join(ROOT, 'assets', 'indexes'),
  assetObjects: path.join(ROOT, 'assets', 'objects'),
  natives: path.join(ROOT, 'natives'),
  runtimes: path.join(ROOT, 'runtimes'),
  instances: path.join(ROOT, 'instances'),
  cache: path.join(ROOT, 'cache'),
  covers: path.join(ROOT, 'covers'),
  logs: path.join(ROOT, 'logs'),
  config: path.join(ROOT, 'config.json'),
  accounts: path.join(ROOT, 'accounts.json'),
};

function ensureDirs() {
  for (const key of ['root', 'versions', 'libraries', 'assets', 'assetIndexes', 'assetObjects',
    'natives', 'runtimes', 'instances', 'cache', 'covers', 'logs']) {
    fs.mkdirSync(P[key], { recursive: true });
  }
}

/** A profile's user-chosen picture, if it has one. */
function coverFile(profileId) {
  return path.join(P.covers, `${profileId}.png`);
}

/** Directory holding a profile's own saves/mods/options, like a MultiMC instance. */
function instanceDir(profileId) {
  return path.join(P.instances, profileId);
}

module.exports = { ...P, ensureDirs, instanceDir, coverFile };
