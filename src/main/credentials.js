'use strict';

const SECRET_FIELDS = ['accessToken', 'msaRefreshToken'];

function encryptionAvailable(safeStorage) {
  try {
    if (!safeStorage?.isEncryptionAvailable()) return false;
    // Electron's Linux "basic_text" backend is reversible obfuscation, not a
    // keyring. A 0600 file is the honest fallback in that environment.
    return safeStorage.getSelectedStorageBackend?.() !== 'basic_text';
  } catch { return false; }
}

function canDecrypt(safeStorage) {
  try { return Boolean(safeStorage?.isEncryptionAvailable()); } catch { return false; }
}

function serializeAccount(account, safeStorage) {
  const out = { ...account };
  const preservedEnvelope = out.encryptedCredentials;
  delete out.encryptedCredentials;
  delete out.credentialsLocked;
  const secrets = {};
  for (const key of SECRET_FIELDS) {
    if (typeof out[key] === 'string' && out[key] !== '' && out[key] !== '0') secrets[key] = out[key];
    delete out[key];
  }
  if (Object.keys(secrets).length === 0) {
    if (typeof preservedEnvelope === 'string') out.encryptedCredentials = preservedEnvelope;
    return out;
  }
  if (!encryptionAvailable(safeStorage)) return { ...out, ...secrets };
  out.encryptedCredentials = safeStorage.encryptString(JSON.stringify(secrets)).toString('base64');
  return out;
}

function deserializeAccount(account, safeStorage) {
  const out = { ...account };
  const encrypted = out.encryptedCredentials;
  delete out.encryptedCredentials;
  if (!encrypted) return out;
  if (!canDecrypt(safeStorage)) {
    return { ...out, encryptedCredentials: encrypted, credentialsLocked: true };
  }
  const secrets = JSON.parse(safeStorage.decryptString(Buffer.from(encrypted, 'base64')));
  for (const key of SECRET_FIELDS) if (typeof secrets[key] === 'string') out[key] = secrets[key];
  return out;
}

function hasPlaintextCredentials(account) {
  return SECRET_FIELDS.some((key) => typeof account?.[key] === 'string' && account[key] !== '' && account[key] !== '0');
}

module.exports = { SECRET_FIELDS, encryptionAvailable, serializeAccount, deserializeAccount, hasPlaintextCredentials };
