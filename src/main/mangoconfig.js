'use strict';
const fs = require('fs');
const fsp = require('fs/promises');
const path = require('path');
const P = require('./paths');
const modrinth = require('./modrinth');

/**
 * MangoConfig - the in-game settings layer MangoClient brings to every launch.
 *
 * Under the hood it is OneConfig by Polyfrost, taken from Modrinth exactly as
 * published and never rewritten. The only thing MangoClient touches is
 * OneConfig's own `themes.json`, a file OneConfig writes itself, so that its
 * interface comes up in the launcher's accent colour instead of Polyfrost blue.
 *
 * That restraint is deliberate. OneConfig is licensed under the LGPLv3 *and*
 * the Additional Terms Applicable to OneConfig, which reserve the right to
 * alter its notices and branding to Polyfrost:
 *   <https://polyfrost.org/legal/oneconfig/additional-terms>
 * Shipping it unmodified keeps both licences satisfied, keeps the mod's own
 * updates working, and leaves its Modrinth hashes intact for the mod list.
 */

/** What the launcher calls the feature. */
const NAME = 'MangoConfig';
/** What it actually is, shown wherever the name appears. */
const CREDIT = 'OneConfig by Polyfrost';
const HOMEPAGE = 'https://polyfrost.org/oneconfig';
const PROJECT = 'oneconfig';

/** `--brand` from the launcher's stylesheet, in the r,g,b,a order OneConfig stores. */
const ACCENT_RGBA = [0xd9, 0x8a, 0x3d, 0xff];
/** OneConfig's own default accent (Polyfrost blue), left alone once changed. */
const DEFAULT_ACCENT_RGBA = [0x2b, 0x4b, 0xff, 0xff];

/**
 * OneConfig publishes Fabric and Forge builds. Quilt loads Fabric mods
 * natively, so it gets the Fabric one; NeoForge will not load a Forge jar, and
 * vanilla has nothing to load it with, so both are left out rather than handed
 * a build that would crash the game.
 */
const LOADER_BUILD = { fabric: 'fabric', quilt: 'fabric', forge: 'forge' };

function enabledFor(profile, config) {
  if (profile?.mangoConfig === false) return false;      // switched off for this instance
  return config?.mangoConfig !== false;                  // otherwise follow the global setting
}

/**
 * Mirrors OneConfig's ConfigManager: configs live in the active profile's
 * folder, which is `config/` until the player creates a named profile.
 */
function activeConfigDir(gameDir) {
  try {
    const profiles = JSON.parse(fs.readFileSync(path.join(gameDir, 'oneconfig', 'profiles.json'), 'utf8'));
    const active = String(profiles?.activeProfile || '').trim();
    if (active) return path.join(gameDir, 'profiles', active);
  } catch { /* no profiles file yet, so the root config folder it is */ }
  return path.join(gameDir, 'config');
}

function sameColour(value, rgba) {
  const arr = Array.isArray(value) ? value : Array.isArray(value?.rgba) ? value.rgba : null;
  return Array.isArray(arr) && arr.length === 4 && arr.every((n, i) => Number(n) === rgba[i]);
}

/**
 * Seed OneConfig's accent colour with the launcher's own.
 *
 * Only when it has never been set, or is still Polyfrost's default: someone who
 * picked their own colour in game keeps it, launch after launch.
 */
async function applyTheme(profileId) {
  const file = path.join(activeConfigDir(P.instanceDir(profileId)), 'themes.json');

  let current = null;
  try {
    current = JSON.parse(await fsp.readFile(file, 'utf8'));
  } catch { /* first launch, or the player has never opened the theme settings */ }

  const accent = current?.accentColor;
  if (accent && !sameColour(accent, DEFAULT_ACCENT_RGBA)) return false;  // their choice, not ours
  if (sameColour(accent, ACCENT_RGBA)) return false;                     // already ours

  const next = { ...(current || {}), accentColor: ACCENT_RGBA };
  await fsp.mkdir(path.dirname(file), { recursive: true });
  await fsp.writeFile(file, `${JSON.stringify(next, null, 2)}\n`);
  return true;
}

/** The profile's MangoConfig entry, whatever it is currently called. */
function recordFor(profile) {
  return (profile.mods || []).find((m) => m.mangoConfig || m.slug === PROJECT) || null;
}

/**
 * Make sure the instance has MangoConfig before it launches, and that it is
 * the build matching this Minecraft version and loader.
 *
 * Never throws: a missing config layer is a worse reason to refuse a launch
 * than to start without it, so every failure is reported and stepped over.
 * Returns `{state, mods?}`; `mods` is set only when the profile needs saving.
 */
async function ensure({ profile, config, onLog = () => {} }) {
  if (!enabledFor(profile, config)) return { state: 'off' };

  const loader = LOADER_BUILD[profile.loader];
  if (!loader) return { state: 'unsupported', reason: profile.loader };

  let version;
  try {
    version = modrinth.pickVersion(
      await modrinth.getVersions(PROJECT, { loader, gameVersion: profile.mcVersion }),
    );
  } catch (err) {
    // Offline. A copy already in the mods folder still loads, so say so and go.
    return { state: 'offline', error: err.message };
  }
  if (!version) return { state: 'unsupported', reason: profile.mcVersion };

  const existing = recordFor(profile);
  if (existing && existing.versionId === version.id && fs.existsSync(existing.file)) {
    return { state: 'current', version: existing.versionNumber };
  }

  let installed;
  try {
    onLog(`${NAME} ${version.version_number} for ${profile.mcVersion} ${profile.loader}`);
    installed = await modrinth.installProject({
      profile,
      projectIdOrSlug: PROJECT,
      versionId: version.id,
      onLog,
    });
  } catch (err) {
    return { state: 'failed', error: err.message };
  }

  const mods = [...(profile.mods || [])];
  for (const rec of installed) {
    // The launcher's name on the outside; the project, the file and the update
    // check underneath still point at OneConfig, which is what keeps working.
    const entry = rec.slug === PROJECT
      ? { ...rec, title: NAME, poweredBy: CREDIT, mangoConfig: true }
      : rec;
    const idx = mods.findIndex((m) => m.projectId === entry.projectId);
    if (idx >= 0) {
      if (mods[idx].file !== entry.file) await fsp.unlink(mods[idx].file).catch(() => {});
      mods[idx] = { ...entry, enabled: mods[idx].enabled !== false };
    } else {
      mods.push({ ...entry, enabled: true });
    }
  }

  return { state: existing ? 'updated' : 'installed', version: version.version_number, mods };
}

module.exports = {
  ensure,
  applyTheme,
  enabledFor,
  recordFor,
  NAME,
  CREDIT,
  HOMEPAGE,
  PROJECT,
  ACCENT_RGBA,
  LOADER_BUILD,
};
