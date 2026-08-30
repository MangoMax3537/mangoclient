'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs/promises');
const os = require('node:os');
const path = require('node:path');
const {
  TAG, readServerList, writeServerList, ensureFeaturedServers,
} = require('../src/main/serverlist');

const stringField = (compound, name) => compound.find((entry) => entry.name === name)?.value;
const server = (name, ip, extra = []) => [
  { type: TAG.string, name: 'name', value: name },
  { type: TAG.string, name: 'ip', value: ip },
  ...extra,
];

test('partnered Minecraft servers are deduplicated, starred and kept first', async (t) => {
  const gameDir = await fs.mkdtemp(path.join(os.tmpdir(), 'mango-serverlist-'));
  t.after(() => fs.rm(gameDir, { recursive: true, force: true }));
  const file = path.join(gameDir, 'servers.dat');
  await writeServerList(file, {
    type: TAG.compound,
    name: '',
    value: [{
      type: TAG.list,
      name: 'servers',
      value: {
        elementType: TAG.compound,
        items: [
          server('Other server', 'example.org'),
          server('Old title', 'vincentvanilla.net:25565', [
            { type: TAG.string, name: 'icon', value: 'data:image/png;base64,preserved' },
            { type: TAG.byte, name: 'acceptTextures', value: 1 },
          ]),
        ],
      },
    }],
  });

  const partnered = [{ name: 'VincentVanilla', address: 'vincentvanilla.net' }];
  await ensureFeaturedServers(gameDir, partnered);
  await ensureFeaturedServers(gameDir, partnered);
  const root = await readServerList(file);
  const items = root.value.find((entry) => entry.name === 'servers').value.items;

  assert.equal(items.length, 2);
  assert.equal(stringField(items[0], 'name'), '★ VincentVanilla');
  assert.equal(stringField(items[0], 'ip'), 'vincentvanilla.net');
  assert.equal(stringField(items[0], 'icon'), 'data:image/png;base64,preserved');
  assert.equal(stringField(items[1], 'name'), 'Other server');
});
