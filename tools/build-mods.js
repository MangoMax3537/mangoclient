'use strict';
const fs = require('fs');
const os = require('os');
const path = require('path');
const { spawnSync } = require('child_process');

const root = path.resolve(__dirname, '..');
const mod = path.join(root, 'mod');
const assets = path.join(root, 'src', 'main', 'assets');
const versions = [
  '1.20.5', '1.20.6', '1.21', '1.21.1', '1.21.2', '1.21.3', '1.21.4',
  '1.21.5', '1.21.6', '1.21.7', '1.21.8', '1.21.9', '1.21.10', '1.21.11',
];
// The one place the version is written down is the mod's own build file.
const release = /^mod_version=(.+)$/m.exec(fs.readFileSync(path.join(mod, 'gradle.properties'), 'utf8'))?.[1]?.trim();
if (!release) throw new Error('mod/gradle.properties has no mod_version');
const stage = fs.mkdtempSync(path.join(os.tmpdir(), 'mangoconfig-release-'));

try {
  for (const mc of versions) {
    process.stdout.write(`\nBuilding MangoConfig ${release} for Minecraft ${mc}\n`);
    const result = spawnSync('bash', ['gradlew', 'clean', 'build', `-PmcVer=${mc}`], {
      cwd: mod,
      stdio: 'inherit',
      env: process.env,
    });
    if (result.status !== 0) process.exit(result.status || 1);
    const name = `MangoConfig-${release}+mc${mc}.jar`;
    const source = path.join(mod, 'build', 'libs', name);
    if (!fs.existsSync(source)) throw new Error(`Gradle did not produce ${name}`);
    fs.copyFileSync(source, path.join(stage, name));
  }

  fs.mkdirSync(assets, { recursive: true });
  for (const name of fs.readdirSync(assets)) {
    if (/^MangoConfig-.*\.jar$/i.test(name)) fs.unlinkSync(path.join(assets, name));
  }
  for (const name of fs.readdirSync(stage)) fs.copyFileSync(path.join(stage, name), path.join(assets, name));
  process.stdout.write(`\nStaged ${versions.length} MangoConfig ${release} builds in src/main/assets\n`);
} finally {
  fs.rmSync(stage, { recursive: true, force: true });
}
