'use strict';
const fs = require('fs');
const fsp = require('fs/promises');
const path = require('path');
const { spawn } = require('child_process');
const P = require('./paths');
const { getJSON, downloadFile, fetchWithRetry } = require('./net');

const FABRIC_META = 'https://meta.fabricmc.net/v2';
const QUILT_META = 'https://meta.quiltmc.org/v3';
const NEOFORGE_MAVEN = 'https://maven.neoforged.net/releases/net/neoforged/neoforge';
const FORGE_MAVEN = 'https://maven.minecraftforge.net/net/minecraftforge/forge';

/** Loader versions available for a given Minecraft version, newest first. */
async function listLoaderVersions(loader, mcVersion) {
  switch (loader) {
    case 'fabric': {
      const list = await getJSON(`${FABRIC_META}/versions/loader/${encodeURIComponent(mcVersion)}`);
      return list.map((e) => ({ version: e.loader.version, stable: e.loader.stable }));
    }
    case 'quilt': {
      const list = await getJSON(`${QUILT_META}/versions/loader/${encodeURIComponent(mcVersion)}`);
      return list.map((e) => ({ version: e.loader.version, stable: !/beta|pre/i.test(e.loader.version) }));
    }
    case 'neoforge': {
      const xml = await (await fetchWithRetry(`${NEOFORGE_MAVEN}/maven-metadata.xml`)).text();
      const all = [...xml.matchAll(/<version>([^<]+)<\/version>/g)].map((m) => m[1]);
      // NeoForge encodes the MC version in its own: 1.21.4 -> 21.4.x
      const parts = mcVersion.split('.');
      const prefix = parts[0] === '1'
        ? `${parts[1]}.${parts[2] ?? '0'}.`
        : `${parts[0]}.${parts[1] ?? '0'}.`;
      return all.filter((v) => v.startsWith(prefix)).reverse()
        .map((v) => ({ version: v, stable: !/beta/i.test(v) }));
    }
    case 'forge': {
      const xml = await (await fetchWithRetry(`${FORGE_MAVEN}/maven-metadata.xml`)).text();
      const all = [...xml.matchAll(/<version>([^<]+)<\/version>/g)].map((m) => m[1]);
      return all.filter((v) => v.startsWith(`${mcVersion}-`)).reverse()
        .map((v) => ({ version: v.split('-')[1], stable: true, full: v }));
    }
    default:
      return [];
  }
}

/** Which Minecraft versions a loader supports (used to filter the version picker). */
async function listLoaderGameVersions(loader) {
  if (loader === 'fabric') {
    const list = await getJSON(`${FABRIC_META}/versions/game`);
    return list.map((v) => ({ version: v.version, stable: v.stable }));
  }
  if (loader === 'quilt') {
    const list = await getJSON(`${QUILT_META}/versions/game`);
    return list.map((v) => ({ version: v.version, stable: v.stable }));
  }
  return [];
}

function versionIdFor(loader, mcVersion, loaderVersion) {
  switch (loader) {
    case 'fabric': return `fabric-loader-${loaderVersion}-${mcVersion}`;
    case 'quilt': return `quilt-loader-${loaderVersion}-${mcVersion}`;
    case 'neoforge': return `neoforge-${loaderVersion}`;
    case 'forge': return `${mcVersion}-forge-${loaderVersion}`;
    default: return mcVersion;
  }
}

/** Fabric and Quilt hand us a ready-made version JSON, no processors needed. */
async function installJsonLoader(loader, mcVersion, loaderVersion, onLog) {
  const base = loader === 'fabric' ? FABRIC_META : QUILT_META;
  const url = `${base}/versions/loader/${encodeURIComponent(mcVersion)}/${encodeURIComponent(loaderVersion)}/profile/json`;
  onLog?.(`Fetching ${loader} ${loaderVersion} for ${mcVersion}…`);
  const json = await getJSON(url);
  const id = json.id || versionIdFor(loader, mcVersion, loaderVersion);
  json.id = id;
  const dir = path.join(P.versions, id);
  await fsp.mkdir(dir, { recursive: true });
  await fsp.writeFile(path.join(dir, `${id}.json`), JSON.stringify(json, null, 2));
  return id;
}

/**
 * Forge/NeoForge need binary patching done by their own installer, so we run the
 * official installer jar headlessly against our game root rather than trying to
 * reimplement its processors.
 */
async function installForgeLike(loader, mcVersion, loaderVersion, javaPath, onLog) {
  const id = versionIdFor(loader, mcVersion, loaderVersion);
  const versionJson = path.join(P.versions, id, `${id}.json`);
  if (fs.existsSync(versionJson)) {
    onLog?.(`${loader} ${loaderVersion} already installed.`);
    return id;
  }

  const full = loader === 'neoforge' ? loaderVersion : `${mcVersion}-${loaderVersion}`;
  const url = loader === 'neoforge'
    ? `${NEOFORGE_MAVEN}/${full}/neoforge-${full}-installer.jar`
    : `${FORGE_MAVEN}/${full}/forge-${full}-installer.jar`;

  const jar = path.join(P.cache, `${loader}-${full}-installer.jar`);
  onLog?.(`Downloading ${loader} installer…`);
  await downloadFile(url, jar);

  // The installer refuses to run without a launcher profile file present.
  const profilesFile = path.join(P.root, 'launcher_profiles.json');
  if (!fs.existsSync(profilesFile)) {
    await fsp.writeFile(profilesFile, JSON.stringify({ profiles: {}, version: 3 }, null, 2));
  }
  await fsp.writeFile(path.join(P.root, 'launcher_profiles_microsoft_store.json'),
    JSON.stringify({ profiles: {}, version: 3 }, null, 2)).catch(() => {});

  onLog?.(`Running ${loader} installer (this can take a minute)…`);
  await new Promise((resolve, reject) => {
    const child = spawn(javaPath, ['-jar', jar, '--installClient', P.root], { cwd: P.root });
    child.stdout.on('data', (d) => onLog?.(String(d).trimEnd()));
    child.stderr.on('data', (d) => onLog?.(String(d).trimEnd()));
    child.on('error', reject);
    child.on('close', (code) => code === 0 ? resolve() : reject(new Error(`${loader} installer exited with code ${code}`)));
  });

  if (!fs.existsSync(versionJson)) {
    // Installers occasionally name the folder slightly differently.
    const dirs = await fsp.readdir(P.versions).catch(() => []);
    const found = dirs.find((d) => d.toLowerCase().includes(loader) && d.includes(loaderVersion));
    if (found) return found;
    throw new Error(`${loader} install finished but no version profile was created`);
  }
  return id;
}

/**
 * Ensure the launchable version id for a profile exists on disk.
 * Returns the version id to hand to the installer/launcher.
 */
async function ensureLoader({ loader, mcVersion, loaderVersion, javaPath, onLog }) {
  if (!loader || loader === 'vanilla') return { versionId: mcVersion, loaderVersion: '' };

  let lv = loaderVersion;
  if (!lv) {
    const versions = await listLoaderVersions(loader, mcVersion);
    if (!versions.length) throw new Error(`No ${loader} build is available for Minecraft ${mcVersion}`);
    lv = (versions.find((v) => v.stable) || versions[0]).version;
    onLog?.(`Using latest ${loader} ${lv}`);
  }

  if (loader === 'fabric' || loader === 'quilt') {
    return { versionId: await installJsonLoader(loader, mcVersion, lv, onLog), loaderVersion: lv };
  }
  if (loader === 'neoforge' || loader === 'forge') {
    return { versionId: await installForgeLike(loader, mcVersion, lv, javaPath, onLog), loaderVersion: lv };
  }
  throw new Error(`Unsupported loader: ${loader}`);
}

module.exports = { listLoaderVersions, listLoaderGameVersions, ensureLoader, versionIdFor };
