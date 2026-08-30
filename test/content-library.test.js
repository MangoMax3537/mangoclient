'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const P = require('../src/main/paths');
const {
  targetDirFor, hasExactInstalledVersion, installedDependents, setEnabledWithDependencies,
} = require('../src/main/modrinth');

test('instance content is routed to Minecraft-compatible folders', () => {
  const id = '00000000-0000-4000-8000-000000000000';
  const base = P.instanceDir(id);

  assert.equal(targetDirFor('mod', id), path.join(base, 'mods'));
  assert.equal(targetDirFor('resourcepack', id), path.join(base, 'resourcepacks'));
  assert.equal(targetDirFor('shader', id), path.join(base, 'shaderpacks'));
});

test('required dependencies are reused only at the exact requested version', () => {
  const profile = {
    mods: [{ projectId: 'sodium', versionId: 'sodium-0.8.13' }],
  };

  assert.equal(hasExactInstalledVersion(profile, 'sodium', 'sodium-0.8.13'), true);
  assert.equal(hasExactInstalledVersion(profile, 'sodium', 'sodium-0.8.7'), false);
});

test('dependency trees disable dependents and enable requirements recursively', async (t) => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'mango-deps-'));
  t.after(() => fs.rmSync(dir, { recursive: true, force: true }));
  const record = (projectId, requiredDependencies = []) => {
    const file = path.join(dir, `${projectId}.jar`);
    fs.writeFileSync(file, projectId);
    return {
      projectId, title: projectId, versionId: `${projectId}-compatible`,
      file, filename: `${projectId}.jar`, enabled: true, requiredDependencies,
    };
  };
  let profile = {
    mods: [
      record('sodium'),
      record('iris', [{ projectId: 'sodium', versionId: 'sodium-compatible' }]),
      record('shader-tools', [{ projectId: 'iris', versionId: 'iris-compatible' }]),
    ],
  };

  assert.deepEqual(installedDependents(profile, 'sodium').map((item) => item.projectId), ['iris']);
  const disabled = await setEnabledWithDependencies(profile, 'sodium', false);
  assert.deepEqual(disabled.affected.map((item) => item.projectId), ['shader-tools', 'iris', 'sodium']);
  assert.ok(disabled.mods.every((item) => item.enabled === false && item.file.endsWith('.disabled')));

  profile = { mods: disabled.mods };
  const enabled = await setEnabledWithDependencies(profile, 'shader-tools', true);
  assert.deepEqual(enabled.affected.map((item) => item.projectId), ['sodium', 'iris', 'shader-tools']);
  assert.ok(enabled.mods.every((item) => item.enabled === true && item.file.endsWith('.jar')));
});

test('instance content UI separates types and opens the matching browser', () => {
  const html = fs.readFileSync(path.resolve(__dirname, '../src/renderer/index.html'), 'utf8');
  const app = fs.readFileSync(path.resolve(__dirname, '../src/renderer/js/app.js'), 'utf8');
  const preload = fs.readFileSync(path.resolve(__dirname, '../src/main/preload.js'), 'utf8');
  const css = fs.readFileSync(path.resolve(__dirname, '../src/renderer/css/app.css'), 'utf8');

  assert.match(html, /data-i18n="instance\.content"/);
  assert.match(app, /const CONTENT_TYPES = \['mod', 'resourcepack', 'shader'\]/);
  assert.match(app, /openContentBrowser\(selected\)/);
  assert.match(app, /profileContent\(state\.profile, 'mod'\)\.length/);
  assert.match(preload, /openContentFolder: \(profileId, type\)/);
  assert.match(preload, /checkAllUpdates: \(\) => call\('modrinth:checkAllUpdates'\)/);
  assert.match(app, /checkModUpdatesOnStartup\(\)/);
  assert.match(css, /#itab-mods > \.rows\s*\{\s*padding:\s*0;/);
  assert.match(css, /\.settings-grid\.sectioned\s*\{[^}]*width:\s*100%[^}]*max-width:\s*none/s);
});

test('Discover exposes dependency owners and instance launch progress', () => {
  const html = fs.readFileSync(path.resolve(__dirname, '../src/renderer/index.html'), 'utf8');
  const app = fs.readFileSync(path.resolve(__dirname, '../src/renderer/js/app.js'), 'utf8');
  const i18n = fs.readFileSync(path.resolve(__dirname, '../src/renderer/js/i18n.js'), 'utf8');

  assert.match(html, /data-i18n="mods\.title">Discover<\/h1>/);
  assert.match(html, /id="instance-launch-progress"[^>]+data-launch-progress/);
  assert.match(app, /function dependencyOwners\(profile, projectId\)/);
  assert.match(app, /data-tip-text="\$\{esc\(dependencyHint\)\}"/);
  assert.match(app, /\$\$\('\[data-progress-fill\]'\)/);
  assert.match(i18n, /'mods\.dependencyHint': 'Wird benötigt von: \{names\}'/);
  assert.match(i18n, /'mods\.dependencyHint': 'Required by: \{names\}'/);
});
