'use strict';
const crypto = require('crypto');
const { fetchWithRetry, getJSON } = require('./net');

/**
 * Microsoft sign-in.
 *
 * We use the OAuth2 authorization-code flow against login.live.com with the
 * public client id of the official Minecraft launcher. The newer AAD endpoint
 * (login.microsoftonline.com/consumers) rejects this id outright with
 * "AADSTS700016: Application ... not found in the directory", so the device
 * code flow is not an option without our own Azure app registration.
 *
 * The chain is: Live code -> Live access token -> Xbox Live -> XSTS ->
 * Minecraft token -> Minecraft profile.
 */
const CLIENT_ID = process.env.MANGOCLIENT_MSA_CLIENT_ID || '00000000402b5328';
const REDIRECT_URI = 'https://login.live.com/oauth20_desktop.srf';
// MBI_SSL yields a ticket Xbox Live accepts verbatim as an RpsTicket.
const SCOPE = 'service::user.auth.xboxlive.com::MBI_SSL';

const LIVE_AUTHORIZE = 'https://login.live.com/oauth20_authorize.srf';
const LIVE_TOKEN = 'https://login.live.com/oauth20_token.srf';

const XBL_AUTH = 'https://user.auth.xboxlive.com/user/authenticate';
const XSTS_AUTH = 'https://xsts.auth.xboxlive.com/xsts/authorize';
const MC_LOGIN = 'https://api.minecraftservices.com/authentication/login_with_xbox';
const MC_PROFILE = 'https://api.minecraftservices.com/minecraft/profile';
const MC_ENTITLEMENTS = 'https://api.minecraftservices.com/entitlements/mcstore';

async function postForm(url, params) {
  const res = await fetchWithRetry(url, {
    method: 'POST',
    headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
    body: new URLSearchParams(params).toString(),
  });
  const json = await res.json().catch(() => ({}));
  return { ok: res.ok, status: res.status, json };
}

async function postJSON(url, body, headers = {}) {
  const res = await fetchWithRetry(url, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json', Accept: 'application/json', ...headers },
    body: JSON.stringify(body),
  });
  const json = await res.json().catch(() => ({}));
  if (!res.ok) {
    const err = new Error(json.Message || json.error_description || `HTTP ${res.status} for ${url}`);
    err.body = json;
    err.status = res.status;
    throw err;
  }
  return json;
}

/** The page we point the sign-in window at. */
function authorizeUrl() {
  const params = new URLSearchParams({
    client_id: CLIENT_ID,
    response_type: 'code',
    redirect_uri: REDIRECT_URI,
    scope: SCOPE,
    prompt: 'select_account',
  });
  return `${LIVE_AUTHORIZE}?${params}`;
}

/**
 * Every navigation of the sign-in window runs through here. Returns null while
 * the user is still working through the login pages.
 */
function readRedirect(url) {
  if (!url || !url.startsWith(REDIRECT_URI)) return null;
  const params = new URL(url).searchParams;
  const error = params.get('error');
  if (error) {
    const desc = params.get('error_description') || error;
    if (error === 'access_denied') return { cancelled: true };
    return { error: desc.replace(/\+/g, ' ') };
  }
  const code = params.get('code');
  return code ? { code } : null;
}

async function redeemCode(code) {
  const { ok, json } = await postForm(LIVE_TOKEN, {
    client_id: CLIENT_ID,
    code,
    grant_type: 'authorization_code',
    redirect_uri: REDIRECT_URI,
    scope: SCOPE,
  });
  if (!ok) throw new Error(json.error_description || 'Microsoft sign-in failed');
  return json;
}

async function refreshLiveToken(refreshToken) {
  const { ok, json } = await postForm(LIVE_TOKEN, {
    client_id: CLIENT_ID,
    refresh_token: refreshToken,
    grant_type: 'refresh_token',
    redirect_uri: REDIRECT_URI,
    scope: SCOPE,
  });
  if (!ok) {
    const err = new Error(json.error_description || 'Session expired. Please sign in again.');
    err.needsRelogin = true;
    throw err;
  }
  return json;
}

/** Live access token -> Xbox Live -> XSTS -> Minecraft token -> profile. */
async function msaTokenToMinecraft(liveAccessToken) {
  const xbl = await postJSON(XBL_AUTH, {
    Properties: {
      AuthMethod: 'RPS',
      SiteName: 'user.auth.xboxlive.com',
      // MBI_SSL tickets go in as-is; only AAD tokens need the "d=" prefix.
      RpsTicket: liveAccessToken,
    },
    RelyingParty: 'http://auth.xboxlive.com',
    TokenType: 'JWT',
  });

  let xsts;
  try {
    xsts = await postJSON(XSTS_AUTH, {
      Properties: { SandboxId: 'RETAIL', UserTokens: [xbl.Token] },
      RelyingParty: 'rp://api.minecraftservices.com/',
      TokenType: 'JWT',
    });
  } catch (err) {
    // Microsoft encodes the *reason* an Xbox account can't be used in XErr.
    const xerr = String(err.body?.XErr || '');
    if (xerr === '2148916233') throw new Error('This Microsoft account has no Xbox profile. Create one at xbox.com, then try again.');
    if (xerr === '2148916235') throw new Error('Xbox Live is not available in this account\'s country.');
    if (xerr === '2148916236' || xerr === '2148916237') throw new Error('This account needs adult verification.');
    if (xerr === '2148916238') throw new Error('This is a child account. An adult must add it to a Family first.');
    throw err;
  }

  const uhs = xsts.DisplayClaims?.xui?.[0]?.uhs;
  const mc = await postJSON(MC_LOGIN, { identityToken: `XBL3.0 x=${uhs};${xsts.Token}` });

  const headers = { Authorization: `Bearer ${mc.access_token}` };
  const entitlements = await getJSON(MC_ENTITLEMENTS, { headers }).catch(() => ({ items: [] }));
  const owns = (entitlements.items || []).length > 0;

  const profRes = await fetchWithRetry(MC_PROFILE, { headers });
  if (profRes.status === 404) {
    throw new Error(owns
      ? 'This account owns Minecraft but has no profile yet. Set a username in the Minecraft launcher first.'
      : 'This Microsoft account does not own Minecraft: Java Edition.');
  }
  if (!profRes.ok) throw new Error(`Could not load the Minecraft profile (HTTP ${profRes.status})`);
  const profile = await profRes.json();

  return {
    type: 'microsoft',
    uuid: profile.id,
    name: profile.name,
    accessToken: mc.access_token,
    expiresAt: Date.now() + (mc.expires_in || 86400) * 1000,
    skins: profile.skins || [],
    capes: profile.capes || [],
  };
}

async function loginWithLiveTokens(tokens) {
  const account = await msaTokenToMinecraft(tokens.access_token);
  return { ...account, msaRefreshToken: tokens.refresh_token };
}

/** Returns an account with a fresh Minecraft token, refreshing only if needed. */
async function ensureFreshAccount(account) {
  if (account.type === 'offline') return account;
  // 5-minute skew so a token can't die mid-launch.
  if (account.accessToken && account.expiresAt && account.expiresAt - Date.now() > 5 * 60 * 1000) {
    return account;
  }
  if (!account.msaRefreshToken) {
    const err = new Error('Session expired. Please sign in again.');
    err.needsRelogin = true;
    throw err;
  }
  const refreshed = await refreshLiveToken(account.msaRefreshToken);
  const fresh = await loginWithLiveTokens(refreshed);
  return { ...account, ...fresh };
}

/** Offline accounts use the same UUID scheme the vanilla server uses. */
function offlineAccount(name) {
  const clean = String(name || '').trim();
  if (!/^[A-Za-z0-9_]{3,16}$/.test(clean)) {
    throw new Error('Username must be 3-16 characters (letters, digits, underscore).');
  }
  const md5 = crypto.createHash('md5').update(`OfflinePlayer:${clean}`).digest();
  md5[6] = (md5[6] & 0x0f) | 0x30; // version 3
  md5[8] = (md5[8] & 0x3f) | 0x80; // RFC 4122 variant
  const hex = md5.toString('hex');
  return {
    type: 'offline',
    uuid: hex,
    name: clean,
    accessToken: '0',
    expiresAt: null,
    skins: [],
    capes: [],
  };
}

module.exports = {
  authorizeUrl,
  readRedirect,
  redeemCode,
  loginWithLiveTokens,
  refreshLiveToken,
  ensureFreshAccount,
  offlineAccount,
  CLIENT_ID,
  REDIRECT_URI,
};
