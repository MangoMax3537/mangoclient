'use strict';
const fs = require('fs');
const fsp = require('fs/promises');
const path = require('path');
const P = require('./paths');
const { getBuffer, getJSON } = require('./net');
const { defaultSkinDataUrl } = require('./defaultSkin');

const SESSION = 'https://sessionserver.mojang.com/session/minecraft/profile';

function cacheFile(name) {
  return path.join(P.cache, 'skins', name);
}

async function fetchAsDataUrl(url) {
  const buf = await getBuffer(url);
  return `data:image/png;base64,${buf.toString('base64')}`;
}

/** Decode the base64 texture blob the session server returns. */
function decodeTextures(propertyValue) {
  const json = JSON.parse(Buffer.from(propertyValue, 'base64').toString('utf8'));
  return {
    skinUrl: json.textures?.SKIN?.url || null,
    capeUrl: json.textures?.CAPE?.url || null,
    slim: json.textures?.SKIN?.metadata?.model === 'slim',
  };
}

/**
 * Resolve a player's skin/cape. Prefers the data already on the signed-in
 * account, falls back to the public session server, then to Steve.
 */
async function getSkin(account) {
  const result = { skin: null, cape: null, slim: false, source: 'default' };

  try {
    const active = (account.skins || []).find((s) => s.state === 'ACTIVE');
    if (active?.url) {
      result.skin = await fetchAsDataUrl(active.url);
      result.slim = /slim/i.test(active.variant || '');
      result.source = 'account';
    }
    const cape = (account.capes || []).find((c) => c.state === 'ACTIVE');
    if (cape?.url) result.cape = await fetchAsDataUrl(cape.url);
  } catch { /* fall through to the session server */ }

  if (!result.skin && account.type !== 'offline') {
    try {
      const clean = String(account.uuid).replace(/-/g, '');
      const prof = await getJSON(`${SESSION}/${clean}?unsigned=false`);
      const textures = prof.properties?.find((p) => p.name === 'textures');
      if (textures) {
        const { skinUrl, capeUrl, slim } = decodeTextures(textures.value);
        if (skinUrl) {
          result.skin = await fetchAsDataUrl(skinUrl);
          result.slim = slim;
          result.source = 'session';
        }
        if (capeUrl && !result.cape) result.cape = await fetchAsDataUrl(capeUrl);
      }
    } catch { /* offline or rate-limited */ }
  }

  if (!result.skin) {
    result.skin = defaultSkinDataUrl();
    result.slim = false;
    result.source = 'default';
  }

  // Persist so the card renders instantly on the next start.
  try {
    await fsp.mkdir(path.dirname(cacheFile('x')), { recursive: true });
    await fsp.writeFile(cacheFile(`${account.uuid}.json`), JSON.stringify(result));
  } catch { /* cache is best-effort */ }

  return result;
}

async function getCachedSkin(uuid) {
  try {
    return JSON.parse(await fsp.readFile(cacheFile(`${uuid}.json`), 'utf8'));
  } catch {
    return null;
  }
}

/** Upload a new skin to the signed-in Microsoft account. */
async function uploadSkin(account, filePath, variant = 'classic') {
  if (account.type === 'offline') throw new Error('Offline accounts cannot change their skin.');
  const data = await fsp.readFile(filePath);
  const form = new FormData();
  form.append('variant', variant);
  form.append('file', new Blob([data], { type: 'image/png' }), path.basename(filePath));
  const res = await fetch('https://api.minecraftservices.com/minecraft/profile/skins', {
    method: 'POST',
    headers: { Authorization: `Bearer ${account.accessToken}` },
    body: form,
  });
  if (!res.ok) throw new Error(`Skin upload failed (HTTP ${res.status})`);
  return res.json();
}

module.exports = { getSkin, getCachedSkin, uploadSkin, defaultSkinDataUrl };
