'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');

test('the first valid partner favicon is durable and cannot be replaced', (t) => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'mango-server-icons-'));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  process.env.MANGOCLIENT_HOME = root;
  const modulePath = require.resolve('../src/main/servericons');
  const pathsModule = require.resolve('../src/main/paths');
  delete require.cache[modulePath];
  delete require.cache[pathsModule];
  const icons = require('../src/main/servericons');
  const first = 'data:image/png;base64,Zmlyc3Q=';
  const later = 'data:image/png;base64,bGF0ZXI=';

  assert.equal(icons.remember('VincentVanilla.net:25565', first), first);
  assert.equal(icons.remember('vincentvanilla.net', later), first);
  assert.equal(icons.get('VINCENTVANILLA.NET.'), first);

  delete require.cache[modulePath];
  const reloaded = require('../src/main/servericons');
  assert.equal(reloaded.get('vincentvanilla.net'), first);
  delete process.env.MANGOCLIENT_HOME;
});
