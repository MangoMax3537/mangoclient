'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');

const main = fs.readFileSync(path.resolve(__dirname, '../src/main/index.js'), 'utf8');
const store = fs.readFileSync(path.resolve(__dirname, '../src/main/store.js'), 'utf8');

test('manual mods are synchronized before initial state and after profile selection', () => {
  assert.match(main, /handle\('app:state',[\s\S]*?syncSelectedMods\(\);[\s\S]*?return \{/);
  assert.match(main, /handle\('profile:select',[\s\S]*?selectedProfile: id[\s\S]*?syncSelectedMods\(\);/);
});

test('legacy inline mod icons are migrated before profiles are validated', () => {
  const migrate = store.indexOf('externalizeInlineIcons(rawProfiles)');
  const validate = store.indexOf('this.profiles = rawProfiles.flatMap');
  assert.notEqual(migrate, -1);
  assert.notEqual(validate, -1);
  assert.ok(migrate < validate);
});
