'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const { serializeAccount, deserializeAccount, hasPlaintextCredentials } = require('../src/main/credentials');

const safeStorage = {
  isEncryptionAvailable: () => true,
  encryptString: (text) => Buffer.from(`encrypted:${text}`),
  decryptString: (buffer) => buffer.toString().replace(/^encrypted:/, ''),
};

test('credentials migrate to an encrypted envelope and round trip', () => {
  const legacy = { uuid: 'a'.repeat(32), name: 'Player', accessToken: 'mc-secret', msaRefreshToken: 'refresh-secret' };
  assert.equal(hasPlaintextCredentials(legacy), true);
  const stored = serializeAccount(legacy, safeStorage);
  assert.equal(stored.accessToken, undefined);
  assert.equal(stored.msaRefreshToken, undefined);
  assert.match(stored.encryptedCredentials, /^[A-Za-z0-9+/]+=*$/);
  assert.deepEqual(deserializeAccount(stored, safeStorage), legacy);
});

test('unavailable key storage retains the 0600-compatible plaintext fallback', () => {
  const unavailable = { isEncryptionAvailable: () => false };
  const account = { uuid: 'b'.repeat(32), accessToken: 'secret' };
  assert.deepEqual(serializeAccount(account, unavailable), account);
});

test('Linux basic_text is treated as a 0600 fallback, while existing envelopes remain readable', () => {
  const basicText = { ...safeStorage, getSelectedStorageBackend: () => 'basic_text' };
  const account = { uuid: 'c'.repeat(32), accessToken: 'secret' };
  assert.deepEqual(serializeAccount(account, basicText), account);
  const encrypted = { uuid: account.uuid, encryptedCredentials: safeStorage.encryptString('{"accessToken":"secret"}').toString('base64') };
  assert.deepEqual(deserializeAccount(encrypted, basicText), account);
});

test('temporarily locked encrypted credentials are preserved losslessly', () => {
  const unavailable = { isEncryptionAvailable: () => false };
  const stored = { uuid: 'd'.repeat(32), name: 'Player', encryptedCredentials: Buffer.from('opaque').toString('base64') };
  const runtime = deserializeAccount(stored, unavailable);
  assert.equal(runtime.credentialsLocked, true);
  assert.deepEqual(serializeAccount(runtime, unavailable), stored);
});
