'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const fsp = require('node:fs/promises');
const os = require('node:os');
const path = require('node:path');

test('a stale content record cannot make its whole instance disappear', async (t) => {
  const root = await fsp.mkdtemp(path.join(os.tmpdir(), 'mango-store-'));
  t.after(() => fsp.rm(root, { recursive: true, force: true }));
  process.env.MANGOCLIENT_HOME = root;
  const id = '12345678-1234-1234-1234-123456789abc';
  fs.writeFileSync(path.join(root, 'accounts.json'), JSON.stringify({
    accounts: [],
    profiles: [{
      id,
      name: 'My preserved instance',
      mcVersion: '1.21.11',
      loader: 'fabric',
      mods: [{ title: 'old record', filename: 'old.jar', file: path.join(root, 'old-layout', 'old.jar') }],
      created: 10,
    }],
  }));

  const { Store } = require('../src/main/store');
  const store = new Store();
  assert.equal(store.profiles.length, 1);
  assert.equal(store.profiles[0].id, id);
  assert.equal(store.profiles[0].name, 'My preserved instance');
  assert.deepEqual(store.profiles[0].mods, []);
});
