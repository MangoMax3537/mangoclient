'use strict';
const fs = require('fs');
const path = require('path');
const crypto = require('crypto');
const AdmZip = require('adm-zip');
const P = require('./paths');
const mangoconfig = require('./mangoconfig');
const performancemods = require('./performancemods');
const { LIMITS } = require('./archive');

/**
 * Mods the player dropped into the instance's `mods` folder by hand.
 *
 * Everything the launcher installs itself is recorded on the profile, but a jar
 * copied in from outside has no record, so it would run in game and still be
 * missing from the list. This module reconciles the profile with what is
 * actually on disk: unknown jars become "local" entries, and entries whose jar
 * was deleted by hand disappear.
 */

const JAR_RE = /\.jar(\.disabled)?$/i;
/** A logo big enough to matter is not worth inlining into the profile file. */
const MAX_ICON_BYTES = 256 * 1024;

/** `foo.jar.disabled` -> `foo.jar`, so a mod keeps its identity when toggled. */
function jarName(name) {
  return name.replace(/\.disabled$/i, '');
}

function imageUrl(file) {
  return `mangoimg://f/${Buffer.from(file, 'utf8').toString('base64url')}`;
}

function iconFileUrl(zip, logoFile) {
  if (!logoFile || !/\.png$/i.test(logoFile)) return null;
  const entry = zip.getEntry(logoFile.replace(/^\/+/, ''));
  if (!entry || entry.isDirectory || entry.header.size > MAX_ICON_BYTES) return null;
  try {
    const data = entry.getData();
    const hash = crypto.createHash('sha1').update(data).digest('hex');
    const dir = path.join(P.cache, 'mod-icons');
    const file = path.join(dir, `${hash}.png`);
    fs.mkdirSync(dir, { recursive: true });
    if (!fs.existsSync(file)) fs.writeFileSync(file, data);
    return imageUrl(file);
  } catch {
    return null;
  }
}

function readEntry(zip, name) {
  const entry = zip.getEntry(name);
  if (!entry || entry.isDirectory || entry.header.size > LIMITS.metadata) return null;
  try {
    // Some mods ship their metadata with a byte-order mark, which JSON.parse rejects.
    return entry.getData().toString('utf8').replace(/^﻿/, '');
  } catch {
    return null;
  }
}

/** Forge/NeoForge ship TOML; pulling three fields out beats a parser dependency. */
function parseModsToml(text) {
  const start = text.indexOf('[[mods]]');
  if (start < 0) return null;
  const rest = text.slice(start + '[[mods]]'.length);
  const end = rest.indexOf('[[');
  const block = end < 0 ? rest : rest.slice(0, end);
  const field = (key) => {
    const m = block.match(new RegExp(`^\\s*${key}\\s*=\\s*["'](.*?)["']`, 'm'))
      || block.match(new RegExp(`^\\s*${key}\\s*=\\s*'''([\\s\\S]*?)'''`, 'm'));
    return m ? m[1].trim() : null;
  };
  const logo = text.match(/^\s*logoFile\s*=\s*["'](.*?)["']/m);
  return {
    title: field('displayName') || field('modId'),
    version: field('version'),
    logoFile: logo ? logo[1] : null,
  };
}

/** Forge writes `${file.jarVersion}`, which only the manifest can resolve. */
function manifestVersion(zip) {
  const mf = readEntry(zip, 'META-INF/MANIFEST.MF');
  const m = mf && mf.match(/^Implementation-Version:\s*(.+)$/m);
  return m ? m[1].trim() : null;
}

/** Last resort: most jars are named `sodium-fabric-0.6.0+mc1.21.jar`. */
function versionFromFilename(filename) {
  const m = jarName(filename).replace(/\.jar$/i, '').match(/[-_](v?\d[\w.+]*)$/);
  return m ? m[1] : '';
}

function titleFromFilename(filename) {
  const base = jarName(filename).replace(/\.jar$/i, '');
  const name = base.replace(/[-_]v?\d[\w.+]*$/, '').replace(/[-_]+/g, ' ').trim();
  return name || base;
}

/**
 * Read a jar's own idea of its name, version and logo. Supports Fabric, Quilt,
 * Forge/NeoForge and legacy `mcmod.info`; falls back to the file name.
 */
function readJarMeta(file) {
  const filename = path.basename(file);
  const fallback = { title: titleFromFilename(filename), version: versionFromFilename(filename), icon: null };

  let zip;
  try {
    zip = new AdmZip(file);
  } catch {
    return fallback; // not a readable jar, but the loader may still take it
  }

  try {
    const fabric = readEntry(zip, 'fabric.mod.json');
    if (fabric) {
      // Some mods ship trailing commas or raw newlines in descriptions.
      const data = JSON.parse(fabric.replace(/,(\s*[}\]])/g, '$1'));
      return {
        title: data.name || data.id || fallback.title,
        version: data.version || fallback.version,
        icon: iconFileUrl(zip, typeof data.icon === 'string' ? data.icon : data.icon?.['64']),
      };
    }

    const quilt = readEntry(zip, 'quilt.mod.json');
    if (quilt) {
      const loader = JSON.parse(quilt).quilt_loader || {};
      const meta = loader.metadata || {};
      return {
        title: meta.name || loader.id || fallback.title,
        version: loader.version || fallback.version,
        icon: iconFileUrl(zip, typeof meta.icon === 'string' ? meta.icon : meta.icon?.['64']),
      };
    }

    const toml = readEntry(zip, 'META-INF/neoforge.mods.toml') || readEntry(zip, 'META-INF/mods.toml');
    if (toml) {
      const data = parseModsToml(toml) || {};
      let version = data.version;
      if (!version || version.includes('${')) version = manifestVersion(zip) || fallback.version;
      return {
        title: data.title || fallback.title,
        version,
        icon: iconFileUrl(zip, data.logoFile),
      };
    }

    const legacy = readEntry(zip, 'mcmod.info');
    if (legacy) {
      const list = JSON.parse(legacy);
      const first = Array.isArray(list) ? list[0] : list?.modList?.[0];
      if (first) {
        return {
          title: first.name || first.modid || fallback.title,
          version: first.version || fallback.version,
          icon: null,
        };
      }
    }
  } catch { /* malformed metadata: the file name still tells us enough */ }

  return fallback;
}

/** Map of `<name>.jar` -> {file, enabled} for everything in a mods folder. */
function scanModsDir(dir) {
  let entries;
  try {
    entries = fs.readdirSync(dir, { withFileTypes: true });
  } catch {
    return new Map(); // folder not created yet
  }
  const found = new Map();
  // Jars the launcher itself manages - MangoConfig and the performance set -
  // are run by their own switches and have no business in the mod list.
  const managed = performancemods.managedNamesSync(dir);
  for (const entry of entries) {
    if (!entry.isFile() || !JAR_RE.test(entry.name)) continue;
    if (mangoconfig.isOwnJar(entry.name)) continue;
    if (managed.has(entry.name)) continue;
    const key = jarName(entry.name);
    const enabled = !/\.disabled$/i.test(entry.name);
    // If both `foo.jar` and `foo.jar.disabled` exist, the live one wins.
    if (found.get(key)?.enabled) continue;
    found.set(key, { file: path.join(dir, entry.name), enabled });
  }
  return found;
}

function localRecord(filename, info) {
  const meta = readJarMeta(info.file);
  return {
    projectId: `local:${filename}`,
    slug: null,
    title: meta.title,
    type: 'mod',
    icon: meta.icon,
    versionId: null,
    versionNumber: meta.version || '',
    filename,
    file: info.file,
    gameVersions: [],
    loaders: [],
    installedAt: Date.now(),
    dependency: false,
    local: true,          // added by hand, so there is nothing to check updates against
    enabled: info.enabled,
  };
}

/**
 * Reconcile a profile's mod list with its `mods` folder.
 * Returns the list to store and whether anything actually moved.
 */
function syncProfileMods(profile) {
  const dir = path.join(P.instanceDir(profile.id), 'mods');
  const onDisk = scanModsDir(dir);
  const mods = [];
  let changed = false;

  for (const rec of profile.mods || []) {
    // Shaders, resource packs and datapacks live in their own folders.
    if ((rec.type || 'mod') !== 'mod') { mods.push(rec); continue; }

    const filename = rec.filename || jarName(path.basename(rec.file || ''));
    const hit = onDisk.get(filename);
    if (!hit) {
      changed = true; // jar was deleted outside the launcher
      continue;
    }
    onDisk.delete(filename);
    const inlineIcon = rec.local && /^data:image\//.test(rec.icon || '');
    if (rec.file !== hit.file || (rec.enabled !== false) !== hit.enabled || inlineIcon) {
      changed = true;
      const replacement = inlineIcon ? readJarMeta(hit.file).icon : rec.icon;
      mods.push({ ...rec, filename, file: hit.file, enabled: hit.enabled, icon: replacement });
    } else {
      mods.push(rec);
    }
  }

  // Whatever is left was copied in by hand.
  const added = [...onDisk.keys()].sort((a, b) => a.localeCompare(b));
  for (const filename of added) {
    changed = true;
    mods.push(localRecord(filename, onDisk.get(filename)));
  }

  return { mods, changed };
}

module.exports = { syncProfileMods, readJarMeta, scanModsDir, modsDirFor: (id) => path.join(P.instanceDir(id), 'mods') };
