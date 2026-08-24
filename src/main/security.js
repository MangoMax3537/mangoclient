'use strict';
const fsp = require('fs/promises');
const path = require('path');
const { pathToFileURL } = require('url');

const IMAGE_EXTENSIONS = new Set(['.png', '.jpg', '.jpeg', '.webp', '.gif']);
const LEGACY_API = 'http://94.249.184.45:8880';

function rendererUrl(file) {
  return pathToFileURL(path.resolve(file)).toString();
}

function isExactRendererUrl(url, file) {
  try {
    const actual = new URL(String(url));
    const expected = new URL(rendererUrl(file));
    return actual.protocol === 'file:'
      && actual.username === '' && actual.password === ''
      && actual.host === '' && actual.search === ''
      && actual.hash === '' && actual.pathname === expected.pathname;
  } catch {
    return false;
  }
}

function isAllowedExternalUrl(raw) {
  try {
    const url = new URL(String(raw));
    return url.protocol === 'https:' && url.username === '' && url.password === '';
  } catch {
    return false;
  }
}

function isTrustedIpcSender(event, webContents, file) {
  return Boolean(event && webContents && event.sender === webContents
    && event.senderFrame === webContents.mainFrame
    && isExactRendererUrl(event.senderFrame?.url, file));
}

function validatedApiBase(raw, legacy = LEGACY_API) {
  const value = String(raw || legacy).replace(/\/+$/, '');
  let url;
  try { url = new URL(value); } catch { throw new Error('Invalid Mango API URL'); }
  if (url.username || url.password || url.search || url.hash || url.pathname !== '/') {
    throw new Error('Mango API URL must be an origin');
  }
  if (url.protocol !== 'https:' && value !== legacy) {
    throw new Error('Mango API URL must use HTTPS');
  }
  return value;
}

function containsPath(root, target) {
  const rel = path.relative(root, target);
  return rel !== '' && !rel.startsWith(`..${path.sep}`) && rel !== '..' && !path.isAbsolute(rel);
}

async function approvedImagePath(file, roots) {
  if (!file || !IMAGE_EXTENSIONS.has(path.extname(file).toLowerCase())) return null;
  let realFile;
  try {
    realFile = await fsp.realpath(path.resolve(file));
    const stat = await fsp.stat(realFile);
    if (!stat.isFile()) return null;
  } catch {
    return null;
  }
  for (const root of roots) {
    try {
      const realRoot = await fsp.realpath(path.resolve(root));
      if (containsPath(realRoot, realFile)) return realFile;
    } catch { /* an optional image directory may not exist yet */ }
  }
  return null;
}

module.exports = {
  IMAGE_EXTENSIONS, LEGACY_API, rendererUrl, isExactRendererUrl,
  isAllowedExternalUrl, isTrustedIpcSender, validatedApiBase, containsPath, approvedImagePath,
};
