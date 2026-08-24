'use strict';
const fs = require('fs');
const fsp = require('fs/promises');
const path = require('path');
const { execFile } = require('child_process');
const { promisify } = require('util');
const tar = require('tar');
const AdmZip = require('adm-zip');
const P = require('./paths');
const { downloadFile, getJSON } = require('./net');
const { safeArchivePath } = require('./archive');

const execFileAsync = promisify(execFile);

const ADOPTIUM = 'https://api.adoptium.net/v3';
const EXE = process.platform === 'win32' ? 'javaw.exe' : 'java';

const ADOPT_OS = process.platform === 'win32' ? 'windows'
  : process.platform === 'darwin' ? 'mac' : 'linux';
const ADOPT_ARCH = { x64: 'x64', arm64: 'aarch64', ia32: 'x86', arm: 'arm' }[process.arch] || 'x64';

/** Read the major version out of `java -version` output. */
async function probeJava(javaPath) {
  try {
    const { stderr, stdout } = await execFileAsync(javaPath, ['-version'], { timeout: 10000 });
    const out = `${stderr}${stdout}`;
    const m = out.match(/version "(\d+)(?:\.(\d+))?[^"]*"/);
    if (!m) return null;
    // 1.8.0_x reports as "1.8"; 17+ reports as "17".
    const major = m[1] === '1' ? Number(m[2]) : Number(m[1]);
    return { path: javaPath, major, raw: out.split('\n')[0].trim() };
  } catch {
    return null;
  }
}

async function findSystemJavas() {
  const candidates = new Set();
  if (process.env.JAVA_HOME) candidates.add(path.join(process.env.JAVA_HOME, 'bin', EXE));
  candidates.add(EXE === 'java' ? 'java' : 'java.exe');

  for (const base of ['/usr/lib/jvm', '/usr/java', '/opt/java', '/Library/Java/JavaVirtualMachines']) {
    const entries = await fsp.readdir(base, { withFileTypes: true }).catch(() => []);
    for (const e of entries) {
      if (!e.isDirectory() && !e.isSymbolicLink()) continue;
      candidates.add(path.join(base, e.name, 'bin', 'java'));
      candidates.add(path.join(base, e.name, 'Contents', 'Home', 'bin', 'java'));
    }
  }

  const found = [];
  for (const c of candidates) {
    const info = await probeJava(c);
    if (info) found.push(info);
  }
  return found;
}

/** Runtimes we manage ourselves, keyed by major version. */
async function listManagedRuntimes() {
  const out = [];
  const entries = await fsp.readdir(P.runtimes, { withFileTypes: true }).catch(() => []);
  for (const e of entries) {
    if (!e.isDirectory()) continue;
    const exe = await findJavaExecutable(path.join(P.runtimes, e.name));
    if (exe) {
      const info = await probeJava(exe);
      if (info) out.push({ ...info, managed: true, id: e.name });
    }
  }
  return out;
}

/** JREs unpack into a versioned subfolder whose name we can't predict. */
async function findJavaExecutable(root) {
  const direct = path.join(root, 'bin', EXE);
  if (fs.existsSync(direct)) return direct;
  const mac = path.join(root, 'Contents', 'Home', 'bin', EXE);
  if (fs.existsSync(mac)) return mac;

  const entries = await fsp.readdir(root, { withFileTypes: true }).catch(() => []);
  for (const e of entries) {
    if (!e.isDirectory()) continue;
    const nested = await findJavaExecutable(path.join(root, e.name));
    if (nested) return nested;
  }
  return null;
}

async function downloadRuntime(major, onProgress = () => {}, onLog = () => {}) {
  const dir = path.join(P.runtimes, `jre-${major}`);
  const existing = await findJavaExecutable(dir);
  if (existing) return existing;

  onLog(`Downloading Java ${major} runtime (Eclipse Temurin)…`);
  const url = `${ADOPTIUM}/assets/latest/${major}/hotspot`
    + `?architecture=${ADOPT_ARCH}&image_type=jre&os=${ADOPT_OS}&vendor=eclipse`;
  const assets = await getJSON(url);
  if (!Array.isArray(assets) || assets.length === 0) {
    throw new Error(`No Java ${major} runtime is published for ${ADOPT_OS}/${ADOPT_ARCH}`);
  }
  const pkg = assets[0].binary.package;

  await fsp.mkdir(dir, { recursive: true });
  if (typeof pkg.name !== 'string' || path.basename(pkg.name) !== pkg.name) throw new Error('Java API returned an invalid filename');
  const archive = safeArchivePath(P.cache, pkg.name);
  let received = 0;
  await downloadFile(pkg.link, archive, {
    sha256: pkg.checksum, // Adoptium publishes sha256 for every binary
    size: pkg.size,
    onBytes: (n) => {
      received += n;
      onProgress({ phase: 'java', done: received, total: pkg.size, label: `Java ${major}` });
    },
  });

  onLog(`Extracting Java ${major}…`);
  if (archive.endsWith('.zip')) {
    new AdmZip(archive).extractAllTo(dir, true);
  } else {
    await tar.x({ file: archive, cwd: dir });
  }
  await fsp.unlink(archive).catch(() => {});

  const exe = await findJavaExecutable(dir);
  if (!exe) throw new Error('Java runtime extracted but no executable was found');
  if (process.platform !== 'win32') await fsp.chmod(exe, 0o755).catch(() => {});
  onLog(`Java ${major} ready.`);
  return exe;
}

/** Minecraft's own mapping of component name -> major version, with a fallback. */
function requiredMajor(versionJson) {
  const declared = versionJson?.javaVersion?.majorVersion;
  if (declared) return declared;
  const id = versionJson?.id || '';
  // Post-1.21 releases switched to a year.major scheme (26.1, 26.2, …).
  if (/^\d\d\./.test(id)) return 25;
  const m = id.match(/^1\.(\d+)/);
  const minor = m ? Number(m[1]) : 21;
  if (minor >= 20) return 21;
  if (minor >= 18) return 17;
  if (minor >= 17) return 16;
  return 8;
}

/**
 * Resolve the java binary to launch with:
 * user override -> a compatible system JVM -> a managed one -> download.
 */
async function resolveJava(versionJson, { override, onProgress, onLog } = {}) {
  const major = requiredMajor(versionJson);

  if (override) {
    const info = await probeJava(override);
    if (!info) throw new Error(`Configured Java path is not runnable: ${override}`);
    if (info.major < major) {
      throw new Error(`Minecraft ${versionJson.id} needs Java ${major}, but the configured Java is ${info.major}.`);
    }
    return { path: override, major: info.major, source: 'custom' };
  }

  const managed = await listManagedRuntimes();
  const managedHit = managed.find((j) => j.major === major) || managed.find((j) => j.major > major);
  if (managedHit) return { path: managedHit.path, major: managedHit.major, source: 'managed' };

  const system = await findSystemJavas();
  // Prefer an exact match: newer JVMs can break old Forge versions.
  const exact = system.find((j) => j.major === major);
  if (exact) return { path: exact.path, major: exact.major, source: 'system' };
  const newer = system.filter((j) => j.major > major).sort((a, b) => a.major - b.major)[0];
  if (newer && major >= 17) return { path: newer.path, major: newer.major, source: 'system' };

  const downloaded = await downloadRuntime(major, onProgress, onLog);
  return { path: downloaded, major, source: 'downloaded' };
}

module.exports = { resolveJava, findSystemJavas, listManagedRuntimes, downloadRuntime, probeJava, requiredMajor };
