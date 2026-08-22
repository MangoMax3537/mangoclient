'use strict';
const fs = require('fs');
const fsp = require('fs/promises');
const path = require('path');
const P = require('./paths');
const { downloadFile } = require('./net');
const { getVersions, pickVersion } = require('./modrinth');

/**
 * Performance mods - Sodium, Lithium and FerriteCore, put into every Fabric
 * instance on launch the same way MangoConfig is.
 *
 * These three are the well-trodden "just faster" set: a renderer, a game-logic
 * optimiser and a memory saver, none of which change how the game plays. They
 * are resolved from Modrinth once per Minecraft version, cached under the
 * launcher's own roof, and copied in from there - so launch two is offline-safe
 * and launch one is a few small downloads.
 *
 * A manifest in the instance remembers which files are ours. Anything the
 * player installed themselves - including a different build of the same mod,
 * or something that conflicts like OptiFine - wins: we skip rather than stack.
 */

const NAME = 'Performance mods';

const MODS = [
  // `conflict` matches any jar that makes installing ours a bad idea.
  { slug: 'sodium', name: 'Sodium', conflict: /sodium|embeddium|rubidium|optifine|vulkanmod|nvidium/i },
  { slug: 'lithium', name: 'Lithium', conflict: /lithium|canary/i },
  { slug: 'ferrite-core', name: 'FerriteCore', conflict: /ferrite/i },
];

/** Quilt loads Fabric mods natively; nothing else here gets them. */
const LOADERS = ['fabric', 'quilt'];

/** How long a Modrinth version lookup is trusted before asking again. */
const RESOLVE_TTL_MS = 7 * 24 * 60 * 60 * 1000;

const CACHE_DIR = path.join(P.cache, 'performance-mods');
const MANIFEST = '.mangoclient-performance.json';

function enabledFor(profile, config) {
  if (profile?.performanceMods === false) return false;
  return config?.performanceMods !== false;
}

function supports(profile) {
  return LOADERS.includes(profile.loader);
}

function modsDirOf(profileId) {
  return path.join(P.instanceDir(profileId), 'mods');
}

async function readManifest(modsDir) {
  try {
    const data = JSON.parse(await fsp.readFile(path.join(modsDir, MANIFEST), 'utf8'));
    return data && typeof data.files === 'object' ? data : { files: {} };
  } catch {
    return { files: {} };
  }
}

async function writeManifest(modsDir, manifest) {
  await fsp.writeFile(path.join(modsDir, MANIFEST), JSON.stringify(manifest, null, 2));
}

/** The player's own jars: everything in mods/ that the manifest does not own. */
async function foreignJars(modsDir, manifest) {
  let names;
  try {
    names = await fsp.readdir(modsDir);
  } catch {
    return [];
  }
  const ours = new Set(Object.values(manifest.files));
  return names.filter((n) => /\.jar(\.disabled)?$/i.test(n) && !ours.has(n));
}

/**
 * Which file Modrinth currently offers for `slug` on this Minecraft version.
 * Cached on disk so one lookup serves every launch for a week.
 */
async function resolve(slug, gameVersion) {
  const cacheFile = path.join(CACHE_DIR, `${slug}-${gameVersion}.json`);
  try {
    const cached = JSON.parse(await fsp.readFile(cacheFile, 'utf8'));
    if (Date.now() - cached.at < RESOLVE_TTL_MS && cached.file) return cached.file;
  } catch { /* no cache yet */ }

  const versions = await getVersions(slug, { loader: 'fabric', gameVersion });
  const version = pickVersion(versions);
  if (!version) return null;
  const file = version.files.find((f) => f.primary) || version.files[0];
  if (!file) return null;

  const entry = {
    filename: file.filename,
    url: file.url,
    sha1: file.hashes?.sha1 || null,
    size: file.size || null,
  };
  await fsp.mkdir(CACHE_DIR, { recursive: true });
  await fsp.writeFile(cacheFile, JSON.stringify({ at: Date.now(), file: entry }, null, 2));
  return entry;
}

/** Take every manifest-owned file out of the instance again. */
async function removeAll(modsDir, manifest) {
  let removed = 0;
  for (const name of Object.values(manifest.files)) {
    try {
      await fsp.unlink(path.join(modsDir, name));
      removed++;
    } catch { /* already gone, or in use; either way not ours to force */ }
  }
  try {
    await fsp.unlink(path.join(modsDir, MANIFEST));
  } catch { /* fine */ }
  return removed;
}

/**
 * Put the performance mods into the instance, or take them out again.
 *
 * Never throws and never blocks a launch: a mod that cannot be resolved or
 * downloaded is skipped with a log line, and the game starts without it.
 */
async function ensure({ profile, config, onLog = () => {} }) {
  const modsDir = modsDirOf(profile.id);
  const manifest = await readManifest(modsDir);
  const wanted = enabledFor(profile, config) && supports(profile);

  if (!wanted) {
    const removed = await removeAll(modsDir, manifest);
    if (removed) onLog(`${NAME} removed from this profile`);
    return { state: removed ? 'removed' : 'off' };
  }

  const foreign = await foreignJars(modsDir, manifest);
  const installed = [];
  const skipped = [];

  for (const mod of MODS) {
    try {
      const clash = foreign.find((n) => mod.conflict.test(n));
      if (clash) {
        // The player brought their own; drop ours if we had one in.
        const own = manifest.files[mod.slug];
        if (own) {
          await fsp.unlink(path.join(modsDir, own)).catch(() => {});
          delete manifest.files[mod.slug];
        }
        skipped.push(`${mod.name} (you have ${clash})`);
        continue;
      }

      const target = manifest.files[mod.slug]
        ? path.join(modsDir, manifest.files[mod.slug]) : null;
      let file = null;
      try {
        file = await resolve(mod.slug, profile.mcVersion);
      } catch {
        // Offline or Modrinth down: what is already in place stays.
        if (target && fs.existsSync(target)) continue;
        skipped.push(`${mod.name} (offline)`);
        continue;
      }
      if (!file) {
        // No build for this Minecraft version; take an old one out too.
        if (target) await fsp.unlink(target).catch(() => {});
        delete manifest.files[mod.slug];
        skipped.push(`${mod.name} (no build for ${profile.mcVersion})`);
        continue;
      }

      const dest = path.join(modsDir, file.filename);
      if (manifest.files[mod.slug] === file.filename && fs.existsSync(dest)) continue;

      const cached = path.join(CACHE_DIR, file.filename);
      if (!fs.existsSync(cached)) {
        await fsp.mkdir(CACHE_DIR, { recursive: true });
        await downloadFile(file.url, cached, { sha1: file.sha1, size: file.size });
      }
      await fsp.mkdir(modsDir, { recursive: true });
      // An older build has to go first: two jars, one mod id, no launch.
      if (target && manifest.files[mod.slug] !== file.filename) {
        await fsp.unlink(target).catch(() => {});
      }
      await fsp.copyFile(cached, dest);
      manifest.files[mod.slug] = file.filename;
      installed.push(`${mod.name}`);
    } catch (err) {
      skipped.push(`${mod.name} (${err.message})`);
    }
  }

  try {
    await fsp.mkdir(modsDir, { recursive: true });
    await writeManifest(modsDir, manifest);
  } catch { /* the copies still work without the manifest */ }

  if (installed.length) onLog(`${NAME}: ${installed.join(', ')} ready`);
  if (skipped.length) onLog(`${NAME}: skipped ${skipped.join(', ')}`);
  return { state: 'ok', installed, skipped };
}

/** True for a jar this module manages, so mod lists can label or hide it. */
async function isManaged(profileId, filename) {
  const manifest = await readManifest(modsDirOf(profileId));
  return Object.values(manifest.files).includes(path.basename(filename || ''));
}

/** The managed file names in a mods folder, for synchronous callers. */
function managedNamesSync(modsDir) {
  try {
    const data = JSON.parse(fs.readFileSync(path.join(modsDir, MANIFEST), 'utf8'));
    return new Set(Object.values(data?.files || {}));
  } catch {
    return new Set();
  }
}

module.exports = { ensure, enabledFor, supports, isManaged, managedNamesSync, NAME, MODS };
