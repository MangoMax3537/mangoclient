'use strict';
const fs = require('fs');
const fsp = require('fs/promises');
const path = require('path');
const AdmZip = require('adm-zip');
const P = require('./paths');
const { getJSON, downloadFile, pool } = require('./net');
const { LIMITS, safeArchivePath, validateArchiveEntries } = require('./archive');

const API = 'https://api.modrinth.com/v2';
const responseCache = new Map();
const pendingRequests = new Map();
const SEARCH_TTL = 5 * 60 * 1000;
const METADATA_TTL = 15 * 60 * 1000;

/** Modrinth metadata is immutable or slow-changing. Reuse it while the launcher
 * is open and collapse identical simultaneous calls into one network request. */
async function cachedJSON(url, ttl = METADATA_TTL) {
  const now = Date.now();
  const cached = responseCache.get(url);
  if (cached && now - cached.at < ttl) {
    // Refresh insertion order so the cap acts as a small LRU.
    responseCache.delete(url);
    responseCache.set(url, cached);
    return cached.value;
  }
  if (pendingRequests.has(url)) return pendingRequests.get(url);

  const request = getJSON(url)
    .then((value) => {
      responseCache.set(url, { at: Date.now(), value });
      while (responseCache.size > 250) responseCache.delete(responseCache.keys().next().value);
      return value;
    })
    .finally(() => pendingRequests.delete(url));
  pendingRequests.set(url, request);
  return request;
}

/** Loaders that can also load plain-Fabric mods. */
function loaderFacet(loader) {
  if (loader === 'quilt') return ['categories:quilt', 'categories:fabric'];
  if (loader === 'neoforge') return ['categories:neoforge', 'categories:forge'];
  if (loader === 'vanilla') return null;
  return [`categories:${loader}`];
}

function loaderQuery(loader) {
  if (loader === 'quilt') return ['quilt', 'fabric'];
  if (loader === 'neoforge') return ['neoforge', 'forge'];
  return [loader];
}

async function search({
  query = '', loader = 'fabric', gameVersion = '', projectType = 'mod',
  categories = [], limit = 30, offset = 0, index = 'relevance', openSourceOnly = false,
} = {}) {
  const facets = [[`project_type:${projectType}`]];
  if (projectType === 'mod' || projectType === 'modpack' || projectType === 'shader') {
    const lf = loaderFacet(loader);
    if (lf && projectType === 'mod') facets.push(lf);
  }
  if (gameVersion) facets.push([`versions:${gameVersion}`]);
  for (const c of categories) facets.push([`categories:${c}`]);
  if (openSourceOnly) facets.push(['open_source:true']);

  const params = new URLSearchParams({
    query,
    facets: JSON.stringify(facets),
    limit: String(limit),
    offset: String(offset),
    index,
  });
  return cachedJSON(`${API}/search?${params}`, SEARCH_TTL);
}

const getProject = (idOrSlug) => cachedJSON(`${API}/project/${encodeURIComponent(idOrSlug)}`);

function getProjects(ids) {
  if (!ids.length) return Promise.resolve([]);
  return cachedJSON(`${API}/projects?ids=${encodeURIComponent(JSON.stringify(ids))}`);
}

async function getVersions(idOrSlug, { loader, gameVersion } = {}) {
  const params = new URLSearchParams();
  if (loader && loader !== 'vanilla') params.set('loaders', JSON.stringify(loaderQuery(loader)));
  if (gameVersion) params.set('game_versions', JSON.stringify([gameVersion]));
  const qs = params.toString();
  return cachedJSON(`${API}/project/${encodeURIComponent(idOrSlug)}/version${qs ? `?${qs}` : ''}`);
}

const getVersion = (versionId) => cachedJSON(`${API}/version/${encodeURIComponent(versionId)}`);
const getCategories = () => cachedJSON(`${API}/tag/category`, 24 * 60 * 60 * 1000);

/** Pick the best version: newest release that matches, else newest of anything. */
function pickVersion(versions) {
  if (!versions.length) return null;
  const byDate = [...versions].sort((a, b) => new Date(b.date_published) - new Date(a.date_published));
  return byDate.find((v) => v.version_type === 'release') || byDate[0];
}

function primaryFile(version) {
  return version.files.find((f) => f.primary) || version.files[0];
}

function modsDir(profileId) {
  return path.join(P.instanceDir(profileId), 'mods');
}

function targetDirFor(projectType, profileId) {
  const base = P.instanceDir(profileId);
  if (projectType === 'shader') return path.join(base, 'shaderpacks');
  if (projectType === 'resourcepack') return path.join(base, 'resourcepacks');
  if (projectType === 'datapack') return path.join(base, 'datapacks');
  return path.join(base, 'mods');
}

function safeFilename(name) {
  if (typeof name !== 'string' || !name || path.basename(name) !== name || name.includes('\0')) {
    throw new Error('Modrinth returned an invalid filename');
  }
  return name;
}

/** A dependency is reusable only when it is the exact Modrinth build selected
 * by the parent mod. Sharing a project id alone is not enough: Iris, for
 * example, can require an older Sodium build than the newest Sodium release. */
function hasExactInstalledVersion(profile, projectId, versionId) {
  return (profile.mods || []).some((record) =>
    record.projectId === projectId && record.versionId === versionId);
}

function directDependencyRefs(version, versionsByProject = new Map(), projectByVersion = new Map()) {
  const refs = [];
  const seen = new Set();
  for (const dependency of version.dependencies || []) {
    if (dependency.dependency_type !== 'required') continue;
    const projectId = dependency.project_id || projectByVersion.get(dependency.version_id);
    if (!projectId || seen.has(projectId)) continue;
    seen.add(projectId);
    refs.push({
      projectId,
      versionId: dependency.version_id || versionsByProject.get(projectId) || null,
    });
  }
  return refs;
}

/** Fill dependency metadata on profiles created before MangoClient stored the
 * graph. Version records on Modrinth are immutable, so this only needs doing
 * once per installed build. Failures are left unknown rather than guessed. */
async function hydrateDependencyMetadata(profile) {
  const records = profile.mods || [];
  const missing = records.filter((record) =>
    (record.type || 'mod') === 'mod' && !record.local && record.versionId
      && !Array.isArray(record.requiredDependencies));
  if (!missing.length) return { mods: records, changed: false, complete: true };

  const versions = await pool(missing, 6, async (record) => {
    try { return await getVersion(record.versionId); } catch { return null; }
  });
  const versionByRecord = new Map(missing.map((record, index) => [record.projectId, versions[index]]));
  const versionsByProject = new Map(records.filter((record) => record.projectId && record.versionId)
    .map((record) => [record.projectId, record.versionId]));
  const projectByVersion = new Map(records.filter((record) => record.projectId && record.versionId)
    .map((record) => [record.versionId, record.projectId]));

  const unresolvedVersionIds = new Set();
  for (const version of versions) {
    for (const dependency of version?.dependencies || []) {
      if (dependency.dependency_type === 'required' && !dependency.project_id
          && dependency.version_id && !projectByVersion.has(dependency.version_id)) {
        unresolvedVersionIds.add(dependency.version_id);
      }
    }
  }
  const resolvedOwners = await pool([...unresolvedVersionIds], 6, async (versionId) => {
    try { return await getVersion(versionId); } catch { return null; }
  });
  [...unresolvedVersionIds].forEach((versionId, index) => {
    const owner = resolvedOwners[index]?.project_id;
    if (owner) projectByVersion.set(versionId, owner);
  });

  let complete = true;
  let changed = false;
  const mods = records.map((record) => {
    if (!missing.includes(record)) return record;
    const version = versionByRecord.get(record.projectId);
    if (!version) { complete = false; return record; }
    changed = true;
    return {
      ...record,
      requiredDependencies: directDependencyRefs(version, versionsByProject, projectByVersion),
    };
  });
  return { mods, changed, complete };
}

function dependencyProjectIds(record) {
  return (record.requiredDependencies || []).map((dependency) => dependency.projectId);
}

function installedDependents(profile, projectId) {
  return (profile.mods || []).filter((record) => dependencyProjectIds(record).includes(projectId));
}

/** Toggle a complete dependency tree. Requirements are enabled before their
 * consumer; dependents are disabled before the dependency they consume. */
async function setEnabledWithDependencies(profile, projectId, enabled) {
  const byId = new Map((profile.mods || []).map((record) => [record.projectId, record]));
  if (!byId.has(projectId)) return { mods: profile.mods || [], affected: [] };

  const order = [];
  const visited = new Set();
  const visitRequirements = (id) => {
    if (visited.has(id)) return;
    visited.add(id);
    const record = byId.get(id);
    if (!record) throw new Error(`Required dependency ${id} is not installed`);
    for (const dependency of record.requiredDependencies || []) {
      const installed = byId.get(dependency.projectId);
      if (!installed) throw new Error(`Required dependency ${dependency.projectId} is not installed`);
      if (dependency.versionId && installed.versionId !== dependency.versionId) {
        throw new Error(`${record.title} requires a different version of ${installed.title}. Run Check for updates before enabling it.`);
      }
      visitRequirements(dependency.projectId);
    }
    order.push(id);
  };
  const visitDependents = (id) => {
    if (visited.has(id)) return;
    visited.add(id);
    for (const dependent of installedDependents(profile, id)) visitDependents(dependent.projectId);
    order.push(id);
  };
  if (enabled) visitRequirements(projectId);
  else visitDependents(projectId);

  let mods = [...(profile.mods || [])];
  const affected = [];
  for (const id of order) {
    const index = mods.findIndex((record) => record.projectId === id);
    if (index < 0 || (mods[index].enabled !== false) === enabled) continue;
    const workingProfile = { ...profile, mods };
    const newPath = await setModEnabled(workingProfile, id, enabled);
    mods[index] = { ...mods[index], enabled, file: newPath || mods[index].file };
    affected.push({ projectId: id, title: mods[index].title });
  }
  return { mods, affected };
}

/**
 * Walk a mod's required dependencies so a one-click install actually boots.
 * Optional deps are skipped; they're usually integrations the user didn't ask for.
 */
async function resolveDependencies(version, { loader, gameVersion }, seen = new Set(), out = []) {
  for (const dep of version.dependencies || []) {
    if (dep.dependency_type !== 'required') continue;
    const key = dep.project_id || dep.version_id;
    if (!key || seen.has(key)) continue;
    seen.add(key);

    let depVersion = null;
    if (dep.version_id) {
      depVersion = await getVersion(dep.version_id).catch(() => null);
    } else if (dep.project_id) {
      const versions = await getVersions(dep.project_id, { loader, gameVersion }).catch(() => []);
      depVersion = pickVersion(versions);
    }
    if (!depVersion) continue;
    out.push(depVersion);
    await resolveDependencies(depVersion, { loader, gameVersion }, seen, out);
  }
  return out;
}

/**
 * Install a project into a profile, pulling required dependencies with it.
 * Returns the records to store on the profile so updates can be checked later.
 */
async function installProject({
  profile, projectIdOrSlug, versionId = null, withDependencies = true, onLog = () => {},
}) {
  const project = await getProject(projectIdOrSlug);
  const type = project.project_type;

  let version;
  if (versionId) {
    version = await getVersion(versionId);
  } else {
    const versions = await getVersions(project.id, {
      loader: type === 'mod' ? profile.loader : undefined,
      gameVersion: profile.mcVersion,
    });
    version = pickVersion(versions);
    if (!version) {
      throw new Error(`${project.title} has no build for ${profile.loader} ${profile.mcVersion}`);
    }
  }

  const toInstall = [{ project, version }];
  if (withDependencies && type === 'mod') {
    const deps = await resolveDependencies(version, { loader: profile.loader, gameVersion: profile.mcVersion });
    if (deps.length) {
      const projects = await getProjects(deps.map((d) => d.project_id));
      const byId = new Map(projects.map((p) => [p.id, p]));
      for (const d of deps) {
        toInstall.push({ project: byId.get(d.project_id) || { id: d.project_id, title: d.name, project_type: 'mod' }, version: d });
      }
      onLog(`Also installing ${deps.length} required dependenc${deps.length === 1 ? 'y' : 'ies'}`);
    }
  }

  const selectedVersions = new Map(toInstall.map((item) => [item.project.id, item.version.id]));
  const selectedProjects = new Map(toInstall.map((item) => [item.version.id, item.project.id]));
  const installed = [];
  for (const item of toInstall) {
    const isDependency = item.project.id !== project.id;
    if (isDependency && hasExactInstalledVersion(profile, item.project.id, item.version.id)) continue;
    const file = primaryFile(item.version);
    if (!file) continue;
    const dir = targetDirFor(item.project.project_type || 'mod', profile.id);
    const filename = safeFilename(file.filename);
    const dest = safeArchivePath(dir, filename);
    onLog(`Downloading ${item.project.title} ${item.version.version_number}…`);
    await downloadFile(file.url, dest, { sha512: file.hashes?.sha512, sha1: file.hashes?.sha1, size: file.size });
    installed.push({
      projectId: item.project.id,
      slug: item.project.slug,
      title: item.project.title,
      type: item.project.project_type || 'mod',
      icon: item.project.icon_url || null,
      versionId: item.version.id,
      versionNumber: item.version.version_number,
      filename,
      file: dest,
      gameVersions: item.version.game_versions,
      loaders: item.version.loaders,
      installedAt: Date.now(),
      dependency: isDependency,
      requiredDependencies: directDependencyRefs(item.version, selectedVersions, selectedProjects),
    });
  }
  return installed;
}

async function uninstallMod(profile, projectId) {
  const record = (profile.mods || []).find((m) => m.projectId === projectId);
  if (!record) return false;
  await fsp.unlink(record.file).catch(() => {});
  return true;
}

/** Toggle by renaming: Minecraft and loaders ignore files ending in `.disabled`. */
async function setModEnabled(profile, projectId, enabled) {
  const record = (profile.mods || []).find((m) => m.projectId === projectId);
  if (!record) return null;
  const disabledPath = record.file.endsWith('.disabled') ? record.file : `${record.file}.disabled`;
  const enabledPath = record.file.replace(/\.disabled$/, '');
  const from = enabled ? disabledPath : enabledPath;
  const to = enabled ? enabledPath : disabledPath;
  if (fs.existsSync(from) && from !== to) await fsp.rename(from, to);
  return to;
}

/** Check every Modrinth-managed content item for a compatible newer build. */
async function checkUpdates(profile) {
  const hydrated = await hydrateDependencyMetadata(profile);
  const effectiveProfile = { ...profile, mods: hydrated.mods };
  // Hand-added files have no Modrinth project to compare against.
  const content = (effectiveProfile.mods || []).filter((item) =>
    ['mod', 'resourcepack', 'shader'].includes(item.type || 'mod') && !item.local && item.projectId);
  const pinnedVersions = new Map();
  const requiredBy = new Map();
  for (const parent of effectiveProfile.mods || []) {
    if (parent.enabled === false) continue;
    for (const dependency of parent.requiredDependencies || []) {
      if (!dependency.versionId) continue;
      if (!pinnedVersions.has(dependency.projectId)) pinnedVersions.set(dependency.projectId, new Set());
      pinnedVersions.get(dependency.projectId).add(dependency.versionId);
      if (!requiredBy.has(dependency.projectId)) requiredBy.set(dependency.projectId, []);
      requiredBy.get(dependency.projectId).push({
        projectId: parent.projectId,
        title: parent.title,
        versionId: dependency.versionId,
      });
    }
  }
  const results = await pool(content, 6, async (item) => {
    try {
      const type = item.type || 'mod';
      const pins = pinnedVersions.get(item.projectId);
      let latest;
      let compatibility = false;
      if (pins?.size === 1) {
        latest = await getVersion([...pins][0]);
        compatibility = latest.id !== item.versionId;
      } else if (pins?.size > 1) {
        return {
          projectId: item.projectId,
          type,
          title: item.title,
          conflict: true,
          requiredBy: requiredBy.get(item.projectId) || [],
        };
      } else {
        const versions = await getVersions(item.projectId, {
          loader: type === 'mod' ? profile.loader : undefined,
          gameVersion: profile.mcVersion,
        });
        latest = pickVersion(versions);
      }
      if (latest && latest.id !== item.versionId) {
        return {
          projectId: item.projectId,
          type,
          title: item.title,
          from: item.versionNumber,
          to: latest.version_number,
          versionId: latest.id,
          compatibility,
        };
      }
    } catch { /* project may have been deleted */ }
    return null;
  });
  return results.filter(Boolean);
}

/** Install a Modrinth modpack (.mrpack) into a profile. */
async function installModpack({ profile, versionId, onLog = () => {}, onProgress = () => {} }) {
  const version = await getVersion(versionId);
  const file = primaryFile(version);
  const packPath = safeArchivePath(P.cache, safeFilename(file.filename));
  onLog(`Downloading modpack ${file.filename}…`);
  await downloadFile(file.url, packPath, { sha512: file.hashes?.sha512, sha1: file.hashes?.sha1, size: file.size });

  try {
    const zip = new AdmZip(packPath);
    const entries = zip.getEntries();
    if (entries.length > LIMITS.entries) throw new Error('Invalid .mrpack: too many entries');
    const indexEntry = zip.getEntry('modrinth.index.json');
    if (!indexEntry) throw new Error('Invalid .mrpack: modrinth.index.json is missing');
    if (indexEntry.header.size > LIMITS.index) throw new Error('Invalid .mrpack: index is too large');
    const index = JSON.parse(indexEntry.getData().toString('utf8'));

    const gameDir = P.instanceDir(profile.id);
    const files = (index.files || []).filter((f) => f.env?.client !== 'unsupported');
    if (files.length > LIMITS.entries) throw new Error('Invalid .mrpack: too many downloads');

    let done = 0;
    await pool(files, 8, async (f) => {
      const dest = safeArchivePath(gameDir, f.path);
      if (!Array.isArray(f.downloads) || !f.downloads[0]) throw new Error(`No download for ${f.path}`);
      await downloadFile(f.downloads[0], dest, { sha512: f.hashes?.sha512, sha1: f.hashes?.sha1, size: f.fileSize });
      done++;
      onProgress({ phase: 'modpack', done, total: files.length, label: path.basename(f.path) });
    });

    // Overrides ship configs, resource packs and the like.
    const overrides = entries.filter((entry) => ['overrides/', 'client-overrides/']
      .some((prefix) => entry.entryName.startsWith(prefix)));
    validateArchiveEntries(overrides);
    for (const folder of ['overrides', 'client-overrides']) {
      for (const entry of overrides) {
        if (!entry.entryName.startsWith(`${folder}/`) || entry.isDirectory) continue;
        const rel = entry.entryName.slice(folder.length + 1);
        const dest = safeArchivePath(gameDir, rel);
        await fsp.mkdir(path.dirname(dest), { recursive: true });
        await fsp.writeFile(dest, entry.getData(), { flag: 'wx' }).catch(async (err) => {
          if (err.code !== 'EEXIST') throw err;
          const stat = await fsp.lstat(dest);
          if (stat.isSymbolicLink() || !stat.isFile()) throw new Error(`Unsafe override target: ${rel}`);
          await fsp.writeFile(dest, entry.getData());
        });
      }
    }

    const deps = index.dependencies || {};
    const loader = deps['fabric-loader'] ? 'fabric'
      : deps['quilt-loader'] ? 'quilt'
      : deps.neoforge ? 'neoforge'
      : deps.forge ? 'forge' : 'vanilla';

    return {
      name: index.name,
      mcVersion: deps.minecraft,
      loader,
      loaderVersion: deps['fabric-loader'] || deps['quilt-loader'] || deps.neoforge || deps.forge || '',
      fileCount: files.length,
    };
  } finally {
    await fsp.unlink(packPath).catch(() => {});
  }
}

module.exports = {
  search, getProject, getProjects, getVersions, getVersion, getCategories,
  installProject, uninstallMod, setModEnabled, checkUpdates, installModpack,
  modsDir, targetDirFor, pickVersion, hasExactInstalledVersion,
  hydrateDependencyMetadata, installedDependents, setEnabledWithDependencies,
};
