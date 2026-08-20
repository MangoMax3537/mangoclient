'use strict';
const fsp = require('fs/promises');
const path = require('path');
const { shell, clipboard, nativeImage } = require('electron');
const P = require('./paths');

/**
 * The instance's screenshot folder, the way OneLauncher shows it: the game
 * writes into it, the launcher only reads, so nothing here creates state the
 * game does not already own.
 */

const IMAGE_RE = /\.(png|jpe?g)$/i;

function screenshotDir(profileId) {
  return path.join(P.instanceDir(profileId), 'screenshots');
}

/**
 * A renderer <img> cannot load file:// under our CSP, and inlining megabytes
 * of PNG as data URLs would make a gallery crawl. `mangoimg://` streams the
 * file instead; see registerImageProtocol() in index.js.
 */
function imageUrl(file) {
  return `mangoimg://f/${Buffer.from(file, 'utf8').toString('base64url')}`;
}

/** Undo imageUrl(). Returns null for anything that is not one of ours. */
function fileFromUrl(url) {
  const m = /^mangoimg:\/\/f\/([A-Za-z0-9_-]+)\/?$/.exec(String(url || ''));
  if (!m) return null;
  try {
    return Buffer.from(m[1], 'base64url').toString('utf8');
  } catch {
    return null;
  }
}

/** Only paths inside an instance may be served or deleted. */
function isInsideInstances(file) {
  const rel = path.relative(P.instances, path.resolve(file));
  return Boolean(rel) && !rel.startsWith('..') && !path.isAbsolute(rel);
}

/** Newest first: someone opening the gallery is looking for the last shot. */
async function listScreenshots(profileId) {
  const dir = screenshotDir(profileId);
  let names;
  try {
    names = await fsp.readdir(dir);
  } catch {
    return []; // the game has not taken one yet
  }

  const shots = [];
  for (const name of names) {
    if (!IMAGE_RE.test(name)) continue;
    const file = path.join(dir, name);
    try {
      const stat = await fsp.stat(file);
      if (!stat.isFile()) continue;
      shots.push({ name, size: stat.size, taken: stat.mtimeMs, url: imageUrl(file) });
    } catch { /* deleted between readdir and stat */ }
  }
  return shots.sort((a, b) => b.taken - a.taken);
}

function resolveShot(profileId, name) {
  const file = path.join(screenshotDir(profileId), path.basename(name));
  if (!isInsideInstances(file)) throw new Error('Screenshot is outside the instance');
  return file;
}

async function deleteScreenshot(profileId, name) {
  await fsp.unlink(resolveShot(profileId, name));
  return true;
}

async function revealScreenshot(profileId, name) {
  shell.showItemInFolder(resolveShot(profileId, name));
  return true;
}

/** Put the picture itself on the clipboard, not its path: people paste it. */
async function copyScreenshot(profileId, name) {
  const image = nativeImage.createFromPath(resolveShot(profileId, name));
  if (image.isEmpty()) throw new Error('Could not read the screenshot');
  clipboard.writeImage(image);
  return true;
}

async function openFolder(profileId) {
  const dir = screenshotDir(profileId);
  await fsp.mkdir(dir, { recursive: true });
  shell.openPath(dir);
  return dir;
}

module.exports = {
  screenshotDir,
  listScreenshots,
  deleteScreenshot,
  revealScreenshot,
  copyScreenshot,
  openFolder,
  imageUrl,
  fileFromUrl,
  isInsideInstances,
};
