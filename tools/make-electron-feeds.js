#!/usr/bin/env node
// Bridge for MangoClient 1.3.x (Electron): its electron-updater looks for `latest-linux.yml` (AppImage) and `latest.yml`
// (Windows NSIS) on the latest GitHub release and installs the file named there after checking its SHA-512 and size.
// Writing these feeds for a 2.x release lets every 1.3.x install update itself to 2.x once; from then on the 2.x updater
// (Ed25519-signed latest.json) takes over. macOS 1.3.x builds were never signed, so Squirrel.Mac cannot update them:
// no `latest-mac.yml` is written and Mac users reinstall by hand.
//
//   node tools/make-electron-feeds.js <dir-with-bundles> <version>
'use strict';
const crypto = require('crypto');
const fs = require('fs');
const path = require('path');

const [dir, version] = process.argv.slice(2);
if (!dir || !/^\d+\.\d+\.\d+$/.test(version || '')) {
  console.error('usage: make-electron-feeds.js <dir> <x.y.z>');
  process.exit(2);
}

const feeds = [
  { feed: 'latest-linux.yml', match: (n) => n === `MangoClient_${version}_amd64.AppImage` },
  { feed: 'latest.yml', match: (n) => n === `MangoClient_${version}_x64-setup.exe` },
];

const releaseDate = new Date().toISOString();
for (const { feed, match } of feeds) {
  const name = fs.readdirSync(dir).find(match);
  if (!name) {
    console.error(`no bundle for ${feed} in ${dir}`);
    process.exit(1);
  }
  const bytes = fs.readFileSync(path.join(dir, name));
  const sha512 = crypto.createHash('sha512').update(bytes).digest('base64');
  const yml = [
    `version: ${version}`,
    'files:',
    `  - url: ${name}`,
    `    sha512: ${sha512}`,
    `    size: ${bytes.length}`,
    `path: ${name}`,
    `sha512: ${sha512}`,
    `releaseDate: '${releaseDate}'`,
    '',
  ].join('\n');
  fs.writeFileSync(path.join(dir, feed), yml);
  console.log(`${feed}: ${name} (${bytes.length} bytes)`);
}
