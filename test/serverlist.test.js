'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs/promises');
const os = require('node:os');
const path = require('node:path');
const {
  TAG, readServerList, writeServerList, ensureFeaturedServers, seedFeaturedServers,
  cacheFeaturedServerIcons, encodeModifiedUtf8, featuredName,
} = require('../src/main/serverlist');

const stringField = (compound, name) => compound.find((entry) => entry.name === name)?.value;
const NO_ICONS = { get: () => null, remember: () => null };
const server = (name, ip, extra = []) => [
  { type: TAG.string, name: 'name', value: name },
  { type: TAG.string, name: 'ip', value: ip },
  ...extra,
];

test('NBT strings use Java modified UTF-8 bytes', () => {
  assert.equal(encodeModifiedUtf8('A\0🥭').toString('hex'), '41c080eda0beedb5ad');
});

test('partner star uses Mango gold without colouring the server name', () => {
  assert.equal(featuredName('VincentVanilla'), '§6★ §rVincentVanilla');
});

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
  await ensureFeaturedServers(gameDir, partnered, NO_ICONS);
  await ensureFeaturedServers(gameDir, partnered, NO_ICONS);
  const root = await readServerList(file);
  const items = root.value.find((entry) => entry.name === 'servers').value.items;

  assert.equal(items.length, 2);
  assert.equal(stringField(items[0], 'name'), '§6★ §rVincentVanilla');
  assert.equal(stringField(items[0], 'ip'), 'vincentvanilla.net');
  assert.equal(stringField(items[0], 'icon'), 'data:image/png;base64,preserved');
  assert.equal(stringField(items[1], 'name'), 'Other server');
});

test('server list preserves Java modified-UTF names including emoji and NUL', async (t) => {
  const gameDir = await fs.mkdtemp(path.join(os.tmpdir(), 'mango-serverlist-'));
  t.after(() => fs.rm(gameDir, { recursive: true, force: true }));
  const file = path.join(gameDir, 'servers.dat');
  const unusualName = 'Freunde 🥭 \0 privat';
  await writeServerList(file, {
    type: TAG.compound,
    name: '',
    value: [{
      type: TAG.list,
      name: 'servers',
      value: { elementType: TAG.compound, items: [server(unusualName, 'friends.example')] },
    }],
  });

  const root = await readServerList(file);
  assert.equal(stringField(root.value[0].value.items[0], 'name'), unusualName);
});

test('server list recovers from its complete sidecar after primary corruption', async (t) => {
  const gameDir = await fs.mkdtemp(path.join(os.tmpdir(), 'mango-serverlist-'));
  t.after(() => fs.rm(gameDir, { recursive: true, force: true }));
  const file = path.join(gameDir, 'servers.dat');
  await writeServerList(file, {
    type: TAG.compound,
    name: '',
    value: [{
      type: TAG.list,
      name: 'servers',
      value: { elementType: TAG.compound, items: [server('Keep me', 'keep.example')] },
    }],
  });
  await fs.writeFile(file, Buffer.from('truncated'));

  await ensureFeaturedServers(gameDir, [{ name: 'VincentVanilla', address: 'vincentvanilla.net' }], NO_ICONS);
  const recovered = await readServerList(file);
  const items = recovered.value.find((entry) => entry.name === 'servers').value.items;
  assert.equal(stringField(items[0], 'name'), '§6★ §rVincentVanilla');
  assert.equal(stringField(items[1], 'name'), 'Keep me');
});

test('launch seeding never rewrites an existing player server list', async (t) => {
  const gameDir = await fs.mkdtemp(path.join(os.tmpdir(), 'mango-serverlist-'));
  t.after(() => fs.rm(gameDir, { recursive: true, force: true }));
  const file = path.join(gameDir, 'servers.dat');
  const icon = 'data:image/png;base64,cHJlc2VydmVk';
  await writeServerList(file, {
    type: TAG.compound,
    name: '',
    value: [{
      type: TAG.list,
      name: 'servers',
      value: {
        elementType: TAG.compound,
        items: [
          server('Friend one', 'one.example'),
          server('VincentVanilla', 'vincentvanilla.net', [{ type: TAG.string, name: 'icon', value: icon }]),
          server('Friend two', 'two.example'),
        ],
      },
    }],
  });
  const before = await fs.readFile(file);
  const remembered = [];
  const iconStore = { get: () => null, remember: (address, value) => remembered.push([address, value]) };

  await seedFeaturedServers(gameDir, [{ name: 'VincentVanilla', address: 'vincentvanilla.net' }], iconStore);

  assert.deepEqual(await fs.readFile(file), before);
  assert.deepEqual(remembered, [['vincentvanilla.net', icon]]);
});

test('a cached partner icon is restored when a new list is bootstrapped', async (t) => {
  const gameDir = await fs.mkdtemp(path.join(os.tmpdir(), 'mango-serverlist-'));
  t.after(() => fs.rm(gameDir, { recursive: true, force: true }));
  const icon = 'data:image/png;base64,Y2FjaGVk';
  const iconStore = { get: () => icon, remember: () => null };

  await seedFeaturedServers(gameDir, [{ name: 'VincentVanilla', address: 'vincentvanilla.net' }], iconStore);
  const root = await readServerList(path.join(gameDir, 'servers.dat'));
  const partner = root.value.find((entry) => entry.name === 'servers').value.items[0];

  assert.equal(stringField(partner, 'icon'), icon);
});

test('icon capture only reads the current Minecraft list', async (t) => {
  const gameDir = await fs.mkdtemp(path.join(os.tmpdir(), 'mango-serverlist-'));
  t.after(() => fs.rm(gameDir, { recursive: true, force: true }));
  const file = path.join(gameDir, 'servers.dat');
  await writeServerList(file, {
    type: TAG.compound,
    name: '',
    value: [{ type: TAG.list, name: 'servers', value: {
      elementType: TAG.compound,
      items: [server('VincentVanilla', 'vincentvanilla.net', [
        { type: TAG.string, name: 'icon', value: 'data:image/png;base64,aWNvbg==' },
      ])],
    } }],
  });
  const captured = [];
  await cacheFeaturedServerIcons(gameDir, [{ address: 'vincentvanilla.net' }], {
    get: () => null,
    remember: (address, icon) => captured.push([address, icon]),
  });
  assert.deepEqual(captured, [['vincentvanilla.net', 'data:image/png;base64,aWNvbg==']]);
});
