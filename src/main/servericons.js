'use strict';
const path = require('path');
const P = require('./paths');
const { readJSON, writeJSONAtomic } = require('./persistence');

// This is durable identity data, not disposable download cache. Keeping it at
// the MangoClient root means launcher updates and "clear cache" cannot change
// a partner back to a generated letter tile.
const CACHE_FILE = path.join(P.root, 'server-icons.json');
const MAX_ICON_LENGTH = 2 * 1024 * 1024;
let icons;

function normalizeAddress(address) {
  return String(address || '').trim().toLowerCase().replace(/\.$/, '').replace(/:25565$/, '');
}

function validIcon(value) {
  return typeof value === 'string'
    && value.length <= MAX_ICON_LENGTH
    && /^data:image\/png;base64,[A-Za-z0-9+/]+={0,2}$/.test(value);
}

function load() {
  if (icons) return icons;
  const saved = readJSON(CACHE_FILE, {});
  icons = {};
  for (const [address, icon] of Object.entries(saved && typeof saved === 'object' ? saved : {})) {
    if (validIcon(icon)) icons[normalizeAddress(address)] = icon;
  }
  return icons;
}

function get(address) {
  return load()[normalizeAddress(address)] || null;
}

/** Keep the first real favicon we see. A temporary status change or an
 * unreachable ping must not replace the partner's established identity. */
function remember(address, icon) {
  const key = normalizeAddress(address);
  if (!key || !validIcon(icon)) return get(key);
  const cache = load();
  if (cache[key]) return cache[key];
  cache[key] = icon;
  writeJSONAtomic(CACHE_FILE, cache);
  return icon;
}

module.exports = { get, remember, normalizeAddress, validIcon, CACHE_FILE };
