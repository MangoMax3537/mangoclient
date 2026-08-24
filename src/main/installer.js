'use strict';
const fs = require('fs');
const fsp = require('fs/promises');
const path = require('path');
const os = require('os');
const P = require('./paths');
const { getJSON, downloadFile, pool, isValid } = require('./net');

const VERSION_MANIFEST = 'https://launchermeta.mojang.com/mc/game/version_manifest_v2.json';
const RESOURCES = 'https://resources.download.minecraft.net';

const OS_NAME = process.platform === 'win32' ? 'windows'
  : process.platform === 'darwin' ? 'osx' : 'linux';
const OS_ARCH = process.arch === 'x64' ? 'x86_64'
  : process.arch === 'ia32' ? 'x86'
  : process.arch === 'arm64' ? 'arm64' : process.arch;

let manifestCache = null;
async function getVersionManifest(force = false) {
  if (manifestCache && !force) return manifestCache;
  const cacheFile = path.join(P.cache, 'version_manifest.json');
  if (!force) {
    const cached = await Promise.all([
      fsp.readFile(cacheFile, 'utf8'),
      fsp.stat(cacheFile),
    ]).catch(() => null);
    if (cached && Date.now() - cached[1].mtimeMs < 6 * 60 * 60 * 1000) {
      manifestCache = JSON.parse(cached[0]);
      return manifestCache;
    }
  }
  try {
    manifestCache = await getJSON(VERSION_MANIFEST);
    // Caching is a nicety for offline starts, so never let it fail the fetch.
    await fsp.mkdir(P.cache, { recursive: true })
      .then(() => fsp.writeFile(cacheFile, JSON.stringify(manifestCache)))
      .catch(() => {});
  } catch (err) {
    // Offline: fall back to the last manifest we saw so installed versions still launch.
    const cached = await fsp.readFile(cacheFile, 'utf8').catch(() => null);
    if (!cached) throw err;
    manifestCache = JSON.parse(cached);
  }
  return manifestCache;
}

// ---- rules -----------------------------------------------------------------

function matchOsRule(osRule) {
  if (!osRule) return true;
  if (osRule.name && osRule.name !== OS_NAME) return false;
  if (osRule.arch && osRule.arch !== (process.arch === 'ia32' ? 'x86' : process.arch)) return false;
  if (osRule.version && !new RegExp(osRule.version).test(os.release())) return false;
  return true;
}

/**
 * Mojang rules are evaluated top-to-bottom, last match wins, default deny when
 * any rule exists. `features` covers things like is_demo / has_custom_resolution.
 */
function rulesAllow(rules, features = {}) {
  if (!rules || rules.length === 0) return true;
  let allowed = false;
  for (const rule of rules) {
    let applies = matchOsRule(rule.os);
    if (applies && rule.features) {
      for (const [key, want] of Object.entries(rule.features)) {
        if (Boolean(features[key]) !== Boolean(want)) { applies = false; break; }
      }
    }
    if (applies) allowed = rule.action === 'allow';
  }
  return allowed;
}

/** `group:artifact:version[:classifier]` -> `group/path/artifact/version/file.jar` */
function mavenToPath(name, extraExt) {
  const [coords, ext = extraExt || 'jar'] = name.split('@');
  const parts = coords.split(':');
  const [group, artifact, version] = parts;
  const classifier = parts[3];
  const file = classifier
    ? `${artifact}-${version}-${classifier}.${ext}`
    : `${artifact}-${version}.${ext}`;
  return path.join(group.replace(/\./g, '/'), artifact, version, file);
}

function nativeClassifier(lib) {
  if (!lib.natives) return null;
  const raw = lib.natives[OS_NAME];
  if (!raw) return null;
  return raw.replace('${arch}', process.arch === 'x64' ? '64' : '32');
}

/** True for the modern style where the native is its own `:natives-linux` library. */
function isModernNative(lib) {
  return !lib.natives && /:natives-/.test(lib.name || '');
}

/** Every spelling of the architecture we are running on. */
const ARCH_ALIASES = {
  x64: ['x64', 'x86_64', 'x86-64', 'amd64'],
  ia32: ['x86', 'x86_32', 'i386'],
  arm64: ['arm64', 'aarch64'],
  arm: ['arm', 'arm32'],
};
const CURRENT_ARCHES = ARCH_ALIASES[process.arch] || [process.arch];
const KNOWN_ARCHES = new Set(Object.values(ARCH_ALIASES).flat());

/**
 * Split `…:natives-windows-arm64` into its os/arch parts. Mojang's rules only
 * name the OS for these libraries (all three Windows variants carry
 * `{"os":{"name":"windows"}}`), so the architecture lives in the classifier and
 * nowhere else. A trailing word that is not an architecture — macOS ships a
 * `natives-macos-patch` library — belongs to the OS variant, not the arch.
 */
function parseNativeClassifier(name) {
  const m = /:natives-([a-z0-9]+)(?:-([a-z0-9_]+))?$/.exec(name || '');
  if (!m) return null;
  const suffix = m[2] || null;
  return {
    os: m[1] === 'macos' ? 'osx' : m[1],
    arch: suffix && KNOWN_ARCHES.has(suffix) ? suffix : null,
  };
}

/** Group key for "the same native library, same OS, different architecture". */
function nativeVariantKey(name, info) {
  return `${name.split(':').slice(0, 3).join(':')}@${info.os}`;
}

// ---- version json ----------------------------------------------------------

/**
 * Everything but the version: `group:artifact[:classifier]`. Two entries with
 * the same key are the same library at different versions - exactly the
 * duplicate the JVM must not see twice (Fabric refuses to boot on two ASMs).
 */
function libraryKey(name) {
  const parts = String(name || '').split(':');
  return [parts[0], parts[1], ...parts.slice(3)].join(':');
}

function mergeVersionJson(child, parent) {
  const merged = { ...parent, ...child };
  // The loader's libraries win: vanilla 1.21.8 ships ASM 9.6 while Fabric
  // brings 9.10.1, and both on the classpath is a refused launch. Parent
  // entries whose artifact the child already carries are dropped; the
  // parent's own list is left alone (its internal repeats carry OS rules).
  const childLibs = child.libraries || [];
  const childKeys = new Set(childLibs.map((lib) => libraryKey(lib.name)));
  merged.libraries = [
    ...childLibs,
    ...(parent.libraries || []).filter((lib) => !childKeys.has(libraryKey(lib.name))),
  ];

  if (parent.arguments || child.arguments) {
    merged.arguments = {
      game: [...(parent.arguments?.game || []), ...(child.arguments?.game || [])],
      jvm: [...(parent.arguments?.jvm || []), ...(child.arguments?.jvm || [])],
    };
  }
  if (parent.minecraftArguments && !child.minecraftArguments) {
    merged.minecraftArguments = parent.minecraftArguments;
  }
  merged.downloads = child.downloads || parent.downloads;
  merged.assetIndex = child.assetIndex || parent.assetIndex;
  merged.assets = child.assets || parent.assets;
  merged.javaVersion = child.javaVersion || parent.javaVersion;
  merged.logging = child.logging || parent.logging;
  merged.id = child.id;
  delete merged.inheritsFrom;
  return merged;
}

async function readLocalVersionJson(id) {
  const file = path.join(P.versions, id, `${id}.json`);
  const raw = await fsp.readFile(file, 'utf8');
  return JSON.parse(raw);
}

/**
 * Load a version JSON, downloading it from Mojang if we don't have it, and
 * flattening any `inheritsFrom` chain (Fabric/NeoForge inherit from vanilla).
 */
async function resolveVersionJson(id, seen = new Set()) {
  if (seen.has(id)) throw new Error(`Circular version inheritance at ${id}`);
  seen.add(id);

  let json;
  try {
    json = await readLocalVersionJson(id);
  } catch {
    const manifest = await getVersionManifest();
    const entry = manifest.versions.find((v) => v.id === id);
    if (!entry) throw new Error(`Unknown Minecraft version: ${id}`);
    json = await getJSON(entry.url);
    const dir = path.join(P.versions, id);
    await fsp.mkdir(dir, { recursive: true });
    await fsp.writeFile(path.join(dir, `${id}.json`), JSON.stringify(json, null, 2));
  }

  if (json.inheritsFrom) {
    const parent = await resolveVersionJson(json.inheritsFrom, seen);
    return mergeVersionJson(json, parent);
  }
  return json;
}

// ---- install ---------------------------------------------------------------

function collectLibraries(version) {
  const classpath = [];
  const natives = [];
  const seen = new Set();

  // Which native libraries ship a build for exactly this architecture. Anything
  // that does is preferred over the un-suffixed (x64) variant, which is only a
  // fallback for machines Mojang publishes no dedicated build for.
  const hasExactArch = new Set();
  for (const lib of version.libraries || []) {
    if (!isModernNative(lib) || !rulesAllow(lib.rules)) continue;
    const info = parseNativeClassifier(lib.name);
    if (info?.arch && CURRENT_ARCHES.includes(info.arch)) {
      hasExactArch.add(nativeVariantKey(lib.name, info));
    }
  }

  for (const lib of version.libraries || []) {
    if (!rulesAllow(lib.rules)) continue;
    if (seen.has(lib.name)) continue;
    seen.add(lib.name);

    if (isModernNative(lib)) {
      const info = parseNativeClassifier(lib.name);
      // Extraction flattens every archive into one directory, so letting a
      // foreign architecture through means its lwjgl.dll overwrites ours and
      // the JVM dies with "Failed to locate library".
      if (info && info.os !== OS_NAME) continue;
      if (info?.arch && !CURRENT_ARCHES.includes(info.arch)) continue;
      if (info && !info.arch && hasExactArch.has(nativeVariantKey(lib.name, info))) continue;
    }

    const artifact = lib.downloads?.artifact;
    const classifier = nativeClassifier(lib);

    if (classifier) {
      // Legacy layout: the native lives in downloads.classifiers[...]
      const nat = lib.downloads?.classifiers?.[classifier];
      if (nat) {
        natives.push({
          name: lib.name,
          url: nat.url,
          path: path.join(P.libraries, nat.path || mavenToPath(`${lib.name}:${classifier}`)),
          sha1: nat.sha1,
          size: nat.size,
          exclude: lib.extract?.exclude || [],
        });
      }
      // Some legacy libs ship both a native and a normal jar.
      if (artifact) {
        classpath.push({
          name: lib.name,
          url: artifact.url,
          path: path.join(P.libraries, artifact.path || mavenToPath(lib.name)),
          sha1: artifact.sha1,
          size: artifact.size,
        });
      }
      continue;
    }

    const relPath = artifact?.path || mavenToPath(lib.name);
    const entry = {
      name: lib.name,
      url: artifact?.url || `${(lib.url || 'https://libraries.minecraft.net/').replace(/\/?$/, '/')}${relPath.split(path.sep).join('/')}`,
      path: path.join(P.libraries, relPath),
      sha1: artifact?.sha1,
      size: artifact?.size,
      exclude: lib.extract?.exclude || [],
    };

    if (isModernNative(lib)) natives.push(entry);
    else classpath.push(entry);
  }
  return { classpath, natives };
}

/** Extract native .so/.dll/.dylib payloads next to the version, once. */
async function extractNatives(version, natives, onLog) {
  const AdmZip = require('adm-zip');
  const dir = path.join(P.natives, version.id);
  await fsp.mkdir(dir, { recursive: true });
  const stampFile = path.join(dir, '.mangoclient-stamp');
  const stamp = natives.map((n) => `${n.name}:${n.sha1 || ''}`).sort().join('|');
  const existing = await fsp.readFile(stampFile, 'utf8').catch(() => null);
  if (existing === stamp) return dir;

  // Start from an empty directory: a leftover library from an earlier, wrongly
  // selected set would still be found by java.library.path.
  await fsp.rm(dir, { recursive: true, force: true }).catch(() => {});
  await fsp.mkdir(dir, { recursive: true });

  for (const nat of natives) {
    try {
      const zip = new AdmZip(nat.path);
      for (const entry of zip.getEntries()) {
        if (entry.isDirectory) continue;
        const name = entry.entryName;
        if (name.startsWith('META-INF/')) continue;
        if ((nat.exclude || []).some((ex) => name.startsWith(ex))) continue;
        if (!/\.(so|dll|dylib|jnilib)$/i.test(name)) continue;
        const out = path.join(dir, path.basename(name));
        await fsp.writeFile(out, entry.getData());
      }
    } catch (err) {
      onLog?.(`Could not extract natives from ${nat.name}: ${err.message}`);
    }
  }
  await fsp.writeFile(stampFile, stamp);
  return dir;
}

/**
 * Download everything a version needs. `onProgress({phase, done, total, label})`
 * is called continuously so the UI can show a real progress bar.
 */
async function installVersion(versionId, { onProgress = () => {}, onLog = () => {}, concurrency = 12 } = {}) {
  const version = await resolveVersionJson(versionId);
  const { classpath, natives } = collectLibraries(version);

  // --- client jar
  onProgress({ phase: 'client', done: 0, total: 1, label: `${version.id} client` });
  const clientJar = path.join(P.versions, version.id, `${version.id}.jar`);
  const clientDl = version.downloads?.client;
  if (clientDl) {
    await downloadFile(clientDl.url, clientJar, { sha1: clientDl.sha1, size: clientDl.size });
  } else if (!fs.existsSync(clientJar)) {
    // Loader-only versions inherit the jar from their parent.
    const parentId = version.inheritsFrom || version.id;
    const parentJar = path.join(P.versions, parentId, `${parentId}.jar`);
    if (fs.existsSync(parentJar)) await fsp.copyFile(parentJar, clientJar);
  }
  onProgress({ phase: 'client', done: 1, total: 1, label: `${version.id} client` });

  // --- libraries + natives
  const libs = [...classpath, ...natives];
  let libDone = 0;
  await pool(libs, concurrency, async (lib) => {
    try {
      await downloadFile(lib.url, lib.path, { sha1: lib.sha1, size: lib.size });
    } catch (err) {
      // A single optional library shouldn't abort the whole install.
      if (!(await isValid(lib.path, lib.sha1, lib.size))) {
        onLog(`Library failed: ${lib.name} (${err.message})`);
      }
    }
    libDone++;
    onProgress({ phase: 'libraries', done: libDone, total: libs.length, label: lib.name });
  });

  const nativesDir = await extractNatives(version, natives, onLog);

  // --- log4j config (fixes the "no log config" spam and CVE mitigations)
  let loggingArg = null;
  const logCfg = version.logging?.client;
  if (logCfg?.file?.url) {
    const dest = path.join(P.assets, 'log_configs', logCfg.file.id);
    await downloadFile(logCfg.file.url, dest, { sha1: logCfg.file.sha1, size: logCfg.file.size }).catch(() => {});
    if (fs.existsSync(dest)) loggingArg = (logCfg.argument || '-Dlog4j.configurationFile=${path}').replace('${path}', dest);
  }

  // --- assets
  const assetIndex = version.assetIndex;
  let assetsIndexId = version.assets || assetIndex?.id || 'legacy';
  if (assetIndex?.url) {
    const indexFile = path.join(P.assetIndexes, `${assetIndex.id}.json`);
    await downloadFile(assetIndex.url, indexFile, { sha1: assetIndex.sha1, size: assetIndex.size });
    assetsIndexId = assetIndex.id;

    const index = JSON.parse(await fsp.readFile(indexFile, 'utf8'));
    const objects = Object.entries(index.objects || {});
    let done = 0;
    onProgress({ phase: 'assets', done: 0, total: objects.length, label: 'assets' });
    await pool(objects, concurrency, async ([name, obj]) => {
      const sub = obj.hash.slice(0, 2);
      const dest = path.join(P.assetObjects, sub, obj.hash);
      try {
        await downloadFile(`${RESOURCES}/${sub}/${obj.hash}`, dest, { sha1: obj.hash, size: obj.size });
        // Pre-1.7 ("legacy") builds read assets from a plain folder tree instead.
        if (index.virtual || index.map_to_resources) {
          const target = path.join(P.assets, index.map_to_resources ? 'resources' : path.join('virtual', assetsIndexId), name);
          if (!fs.existsSync(target)) {
            await fsp.mkdir(path.dirname(target), { recursive: true });
            await fsp.copyFile(dest, target);
          }
        }
      } catch (err) {
        onLog(`Asset failed: ${name} (${err.message})`);
      }
      done++;
      if (done % 25 === 0 || done === objects.length) {
        onProgress({ phase: 'assets', done, total: objects.length, label: name });
      }
    });
  }

  onProgress({ phase: 'done', done: 1, total: 1, label: 'ready' });

  return {
    version,
    clientJar,
    classpath: classpath.map((l) => l.path),
    nativesDir,
    assetsIndexId,
    loggingArg,
    assetsRoot: (assetIndex && !version.assets?.startsWith('pre-1.6')) ? P.assets : P.assets,
    isLegacyAssets: assetsIndexId === 'legacy' || assetsIndexId === 'pre-1.6',
  };
}

async function listInstalledVersions() {
  try {
    const dirs = await fsp.readdir(P.versions, { withFileTypes: true });
    return dirs.filter((d) => d.isDirectory()).map((d) => d.name);
  } catch {
    return [];
  }
}

module.exports = {
  getVersionManifest,
  resolveVersionJson,
  installVersion,
  collectLibraries,
  listInstalledVersions,
  rulesAllow,
  mavenToPath,
  OS_NAME,
  OS_ARCH,
};
