#!/usr/bin/env node
// Writes a MangoClient 1.3.4 data directory with the REAL 1.3.4 code (store.js, serverlist.js, stats.js, telemetry.js, auth.js from the
// `master` branch), so the 2.0 migration test reads exactly what an Electron user has on disk. Only `electron` is stubbed (safeStorage).
//
//   git archive master src/main package.json | tar -x -C /tmp/legacy --exclude=src/main/assets
//   node tools/make-legacy-fixture.js /tmp/legacy /tmp/out
//
// The output is frozen into crates/app/tests/fixtures/legacy-1.3.4/ with the data root replaced by {{ROOT}} (mod records and cover state
// store absolute paths), see crates/app/tests/legacy_migration.rs. Rerun only to regenerate the fixture on purpose.
'use strict';
const fs = require('fs');
const path = require('path');
const zlib = require('zlib');
const Module = require('module');

const [legacyDir, outDir] = process.argv.slice(2);
if (!legacyDir || !outDir) { console.error('usage: make-legacy-fixture.js <legacy-checkout> <out-dir>'); process.exit(2); }
const root = path.resolve(outDir, 'root');
fs.rmSync(outDir, { recursive: true, force: true });
fs.mkdirSync(root, { recursive: true });
process.env.MANGOCLIENT_HOME = root;

// safeStorage as Chromium OSCrypt would behave on Linux with a keyring: an opaque, prefixed blob 2.0 cannot read (forces one re-login).
const fakeSafeStorage = {
  isEncryptionAvailable: () => true,
  getSelectedStorageBackend: () => 'gnome_libsecret',
  encryptString: (s) => Buffer.concat([Buffer.from('v11'), Buffer.from(s, 'utf8').map((b) => b ^ 0x5a)]),
  decryptString: (b) => Buffer.from(b.subarray(3).map((x) => x ^ 0x5a)).toString('utf8'),
};
const origLoad = Module._load;
Module._load = function (req, ...rest) {
  if (req === 'electron') return { safeStorage: fakeSafeStorage, app: { getVersion: () => '1.3.4', isPackaged: true } };
  return origLoad.call(this, req, ...rest);
};

const L = (f) => require(path.join(path.resolve(legacyDir), 'src/main', f));
const P = L('paths.js');
const { Store } = L('store.js');
const { offlineAccount } = L('auth.js');
const { TAG, encodeNbt, ensureFeaturedServers, readServerList, writeServerList } = L('serverlist.js');
const stats = L('stats.js');
const telemetry = L('telemetry.js');

// a 1x1 PNG (test data for cover and screenshot files, never shown as art)
const PNG = Buffer.from('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8/5+hHgAHggJ/PchI7wAAAABJRU5ErkJggg==', 'base64');
const DAY = 86_400_000;
const NOW = Date.UTC(2026, 9, 1, 12, 0, 0); // fixed "today" of the 1.3.4 user

(async () => {
  const store = new Store(); // first run: creates the default "MangoClient 1.21.11" fabric profile
  store.setConfig({ language: 'en', ram: 6144, fullscreen: true, width: 1600, height: 900, keepLauncherOpen: false,
    showSnapshots: true, mangoConfig: true, performanceMods: false, concurrentDownloads: 8, performancePreset: 'quality',
    firstRunDone: true, telemetry: false, javaArgs: '-XX:+UseG1GC' });

  const def = store.profiles[0];
  const vanilla = store.addProfile({ name: 'Vanilla Survival', mcVersion: '1.20.1', loader: 'vanilla', color: '#86b86b', ram: 4096 });
  const neo = store.addProfile({ name: 'Create Ä Ü 🚀', mcVersion: '1.21.1', loader: 'neoforge', loaderVersion: '21.1.77', mangoConfig: false });
  const quilt = store.addProfile({ name: 'Quilt Shaders', mcVersion: '1.20.4', loader: 'quilt', loaderVersion: '0.26.0', javaArgs: '-Dfoo=bar' });

  // mods on the default fabric profile: one Modrinth mod (absolute `file`), one disabled, one local jar, one shader, one resource pack
  const inst = (p, ...rest) => path.join(P.instanceDir(p.id), ...rest);
  const put = (file, data = Buffer.from('PK\u0005\u0006' + '\0'.repeat(18), 'binary')) => { fs.mkdirSync(path.dirname(file), { recursive: true }); fs.writeFileSync(file, data); return file; };
  const sodium = put(inst(def, 'mods', 'sodium-fabric-0.6.13+mc1.21.11.jar'));
  // disabled the 1.3.4 way (modrinth.js setModEnabled): the file gets `.disabled`, `file` follows it, `filename` keeps the jar name
  const lithium = put(inst(def, 'mods', 'lithium-fabric-0.15.0+mc1.21.11.jar.disabled'));
  put(inst(def, 'mods', 'my-local-mod-1.0.jar'));
  const shader = put(inst(def, 'shaderpacks', 'ComplementaryReimagined_r5.4.zip'));
  const pack = put(inst(def, 'resourcepacks', 'FreshAnimations_v1.9.2.zip'));
  store.updateProfile(def.id, {
    mods: [
      { projectId: 'AANobbMI', slug: 'sodium', title: 'Sodium', versionId: 'v6b1Ttop', versionNumber: 'mc1.21.11-0.6.13-fabric', filename: path.basename(sodium), file: sodium, type: 'mod', enabled: true, installedAt: NOW - 30 * DAY, gameVersions: ['1.21.11'], loaders: ['fabric'], requiredDependencies: [] },
      { projectId: 'gvQqBUqZ', slug: 'lithium', title: 'Lithium', versionId: 'qvNsoO3l', versionNumber: 'mc1.21.11-0.15.0-fabric', filename: 'lithium-fabric-0.15.0+mc1.21.11.jar', file: lithium, type: 'mod', enabled: false, installedAt: NOW - 29 * DAY, gameVersions: ['1.21.11'], loaders: ['fabric'] },
      { projectId: 'HVnmMxH1', slug: 'complementary-reimagined', title: 'Complementary Shaders - Reimagined', versionId: 'r5.4', versionNumber: 'r5.4', filename: path.basename(shader), file: shader, type: 'shader', enabled: true, installedAt: NOW - 20 * DAY },
      { projectId: 'YAGZ1cCU', slug: 'fresh-animations', title: 'Fresh Animations', versionId: 'v1.9.2', versionNumber: '1.9.2', filename: path.basename(pack), file: pack, type: 'resourcepack', enabled: true, installedAt: NOW - 10 * DAY },
    ],
    lastPlayed: NOW - 2 * DAY, playTimeMs: 51 * 3_600_000,
  });
  store.updateProfile(vanilla.id, { cover: true, lastPlayed: NOW - 400 * DAY, playTimeMs: 3_600_000 });
  fs.mkdirSync(P.covers, { recursive: true });
  fs.writeFileSync(P.coverFile(vanilla.id), PNG);

  // worlds (level.dat as Minecraft writes it: gzip NBT) and screenshots
  const level = (name) => zlib.gzipSync(encodeNbt({ type: TAG.compound, name: '', value: [
    { type: TAG.compound, name: 'Data', value: [
      { type: TAG.string, name: 'LevelName', value: name },
      { type: TAG.long, name: 'LastPlayed', value: BigInt(NOW - 3 * DAY) },
      { type: TAG.int, name: 'GameType', value: 0 },
      { type: TAG.byte, name: 'hardcore', value: 0 },
      { type: TAG.compound, name: 'Version', value: [{ type: TAG.string, name: 'Name', value: '1.21.11' }] },
    ] },
  ] }));
  put(inst(def, 'saves', 'New World', 'level.dat'), level('My Survival World'));
  put(inst(def, 'saves', 'New World', 'region', 'r.0.0.mca'), Buffer.alloc(8192));
  put(inst(def, 'saves', 'Ünïcødé Wörld', 'level.dat'), level('Ünïcødé Wörld ★'));
  put(inst(vanilla, 'saves', 'Old Map', 'level.dat'), level('Old Map'));
  put(inst(def, 'screenshots', '2026-09-28_20.11.05.png'), PNG);
  put(inst(def, 'screenshots', '2026-09-29_21.42.17.png'), PNG);
  put(inst(def, 'options.txt'), Buffer.from('version:4189\nfov:0.25\nlang:de_de\nkey_key.sprint:key.keyboard.left.control\n'));

  // servers.dat: the player's own list (modified UTF-8: NUL, umlauts, an astral emoji), then the 1.3.4 partner seeding on top
  const list = { type: TAG.compound, name: '', value: [{ type: TAG.list, name: 'servers', value: { elementType: TAG.compound, items: [
    [{ type: TAG.string, name: 'name', value: 'Hypixel' }, { type: TAG.string, name: 'ip', value: 'mc.hypixel.net' }, { type: TAG.byte, name: 'acceptTextures', value: 1 }],
    [{ type: TAG.string, name: 'name', value: 'Bäckerei 🍞 Server' }, { type: TAG.string, name: 'ip', value: 'play.example.de:25570' }],
    [{ type: TAG.string, name: 'name', value: 'LAN a\u0000b' }, { type: TAG.string, name: 'ip', value: '192.168.1.40' }],
  ] } }] };
  await writeServerList(inst(def, 'servers.dat'), list);
  await ensureFeaturedServers(P.instanceDir(def.id), [{ id: 'vincentvanilla', name: 'VincentVanilla', address: 'vincentvanilla.net' }],
    { get: () => null, remember: () => {} });
  const check = await readServerList(inst(def, 'servers.dat'));
  if (check.value[0].value.items.length !== 4) throw new Error('unexpected servers.dat');

  // accounts: an offline one and a Microsoft one whose tokens 1.3.4 put into an OSCrypt envelope (encryptedCredentials)
  store.upsertAccount(offlineAccount('Steve_Old'));
  store.upsertAccount({ type: 'microsoft', uuid: '069a79f444e94726a5befca90e38aaf5', name: 'Notch', accessToken: 'fixture-access-token',
    msaRefreshToken: 'fixture-refresh-token', expiresAt: NOW + DAY, skins: [], capes: [] });
  store.setConfig({ selectedProfile: neo.id });

  // the stats journal and the telemetry install id
  const realNow = Date.now;
  Date.now = () => NOW;
  stats.record(def.id, 2 * 3_600_000, NOW - 2 * DAY);
  stats.record(def.id, 45 * 60_000, NOW - DAY);
  stats.record(quilt.id, 20 * 60_000, NOW - 5 * DAY);
  Date.now = realNow;
  const installId = telemetry.installId();

  // freeze: absolute root -> {{ROOT}} in every JSON file (JSON-escaped, so Windows backslashes match too)
  const esc = JSON.stringify(root).slice(1, -1);
  for (const f of ['accounts.json', 'accounts.json.bak', 'config.json', 'config.json.bak', 'stats.json', 'stats.json.bak']) {
    const file = path.join(root, f);
    if (fs.existsSync(file)) fs.writeFileSync(file, fs.readFileSync(file, 'utf8').split(esc).join('{{ROOT}}'));
  }
  fs.writeFileSync(path.join(outDir, 'expected.json'), JSON.stringify({
    generatedBy: 'MangoClient 1.3.4 (master) store.js/serverlist.js/stats.js/telemetry.js via tools/make-legacy-fixture.js',
    installId, selectedProfile: neo.id,
    profiles: { default: def.id, vanilla: vanilla.id, neoforge: neo.id, quilt: quilt.id },
    offlineUuid: offlineAccount('Steve_Old').uuid,
  }, null, 2) + '\n');
  console.log('fixture written to', outDir);
})().catch((e) => { console.error(e); process.exit(1); });
