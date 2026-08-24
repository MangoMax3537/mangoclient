'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs/promises');
const os = require('node:os');
const path = require('node:path');
const { pathToFileURL } = require('node:url');
const {
  isExactRendererUrl, isAllowedExternalUrl, isTrustedIpcSender,
  validatedApiBase, containsPath, approvedImagePath,
} = require('../src/main/security');
const P = require('../src/main/paths');

test('navigation and external URL policies are exact', () => {
  const renderer = path.join(os.tmpdir(), 'renderer', 'index.html');
  const exact = pathToFileURL(renderer).toString();
  assert.equal(isExactRendererUrl(exact, renderer), true);
  assert.equal(isExactRendererUrl(`${exact}#other`, renderer), false);
  assert.equal(isExactRendererUrl(pathToFileURL(`${renderer}.evil`).toString(), renderer), false);
  assert.equal(isAllowedExternalUrl('https://example.com/path'), true);
  assert.equal(isAllowedExternalUrl('https://user:pass@example.com'), false);
  assert.equal(isAllowedExternalUrl('http://example.com'), false);
  assert.equal(isAllowedExternalUrl('javascript:alert(1)'), false);
});

test('IPC requires both the expected webContents and exact top frame URL', () => {
  const renderer = path.join(os.tmpdir(), 'index.html');
  const frame = { url: pathToFileURL(renderer).toString() };
  const contents = { mainFrame: frame };
  const event = { sender: contents, senderFrame: frame };
  assert.equal(isTrustedIpcSender(event, contents, renderer), true);
  assert.equal(isTrustedIpcSender({ ...event, sender: {} }, contents, renderer), false);
  assert.equal(isTrustedIpcSender({ ...event, senderFrame: { url: 'https://evil.example' } }, contents, renderer), false);
  assert.equal(isTrustedIpcSender({ ...event, senderFrame: { url: frame.url } }, contents, renderer), false);
});

test('API base accepts HTTPS and only the exact legacy HTTP origin', () => {
  assert.equal(validatedApiBase('https://api.mangoclient.gg/'), 'https://api.mangoclient.gg');
  assert.equal(validatedApiBase(), 'http://94.249.184.45:8880');
  assert.throws(() => validatedApiBase('http://example.com'), /HTTPS/);
  assert.throws(() => validatedApiBase('https://example.com/path'), /origin/);
});

test('canonical image policy rejects traversal, sibling prefixes and escaping symlinks', async (t) => {
  const tmp = await fs.mkdtemp(path.join(os.tmpdir(), 'mango-images-'));
  t.after(() => fs.rm(tmp, { recursive: true, force: true }));
  const root = path.join(tmp, 'allowed');
  const sibling = path.join(tmp, 'allowed-evil');
  await fs.mkdir(root); await fs.mkdir(sibling);
  const good = path.join(root, 'shot.png');
  const bad = path.join(sibling, 'shot.png');
  await fs.writeFile(good, 'png'); await fs.writeFile(bad, 'png');
  assert.equal(await approvedImagePath(good, [root]), await fs.realpath(good));
  assert.equal(await approvedImagePath(bad, [root]), null);
  assert.equal(containsPath(root, bad), false);
  const link = path.join(root, 'escape.png');
  await fs.symlink(bad, link);
  assert.equal(await approvedImagePath(link, [root]), null);
  assert.equal(await approvedImagePath(path.join(root, 'note.txt'), [root]), null);
});

test('profile filesystem targets require canonical UUIDs', () => {
  assert.throws(() => P.instanceDir('../outside'), /Invalid profile ID/);
  assert.throws(() => P.coverFile('not-a-uuid'), /Invalid profile ID/);
  const id = '12345678-1234-1234-1234-123456789abc';
  assert.equal(path.basename(P.instanceDir(id)), id);
});
