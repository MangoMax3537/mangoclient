'use strict';
const fs = require('fs');
const fsp = require('fs/promises');
const path = require('path');
const AdmZip = require('adm-zip');
const P = require('./paths');
const { getJSON, downloadFile, pool } = require('./net');
const { LIMITS, safeArchivePath, validateArchiveEntries } = require('./archive');

const API = 'https://api.modrinth.com/v2';

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
  return getJSON(`${API}/search?${params}`);
}

const getProject = (idOrSlug) => getJSON(`${API}/project/${encodeURIComponent(idOrSlug)}`);

function getProjects(ids) {
  if (!ids.length) return Promise.resolve([]);
  return getJSON(`${API}/projects?ids=${encodeURIComponent(JSON.stringify(ids))}`);
}

async function getVersions(idOrSlug, { loader, gameVersion } = {}) {
  const params = new URLSearchParams();
  if (loader && loader !== 'vanilla') params.set('loaders', JSON.stringify(loaderQuery(loader)));
  if (gameVersion) params.set('game_versions', JSON.stringify([gameVersion]));
  const qs = params.toString();
  return getJSON(`${API}/project/${encodeURIComponent(idOrSlug)}/version${qs ? `?${qs}` : ''}`);
}

const getVersion = (versionId) => getJSON(`${API}/version/${encodeURIComponent(versionId)}`);
const getCategories = () => getJSON(`${API}/tag/category`);

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

  const installed = [];
  const already = new Set((profile.mods || []).map((m) => m.projectId));

  for (const item of toInstall) {
    if (already.has(item.project.id) && item.project.id !== project.id) continue;
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
      dependency: item.project.id !== project.id,
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

/** Toggle by renaming: the loader ignores anything not ending in `.jar`. */
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

/** Check every installed mod for a newer build matching the profile. */
async function checkUpdates(profile) {
  // Hand-added jars have no Modrinth project to compare against.
  const mods = (profile.mods || []).filter((m) => m.type === 'mod' && !m.local);
  const results = await pool(mods, 6, async (mod) => {
    try {
      const versions = await getVersions(mod.projectId, { loader: profile.loader, gameVersion: profile.mcVersion });
      const latest = pickVersion(versions);
      if (latest && latest.id !== mod.versionId) {
        return { projectId: mod.projectId, title: mod.title, from: mod.versionNumber, to: latest.version_number, versionId: latest.id };
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
  modsDir, pickVersion,
};
