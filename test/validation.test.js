'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const { configPatch, profilePatch } = require('../src/main/validation');

test('configuration uses typed fields, bounds, and enums', () => {
  assert.deepEqual(configPatch({ language: 'en', ram: 4096, fullscreen: true }), { language: 'en', ram: 4096, fullscreen: true });
  assert.throws(() => configPatch({ __proto__: null, surprise: true }), /Unknown/);
  assert.throws(() => configPatch({ concurrentDownloads: 1000 }), /Invalid/);
  assert.throws(() => configPatch({ performancePreset: 'fastest' }), /Invalid/);
});

test('profile validation rejects mutable ids, unsafe versions, colors and loaders', () => {
  assert.deepEqual(profilePatch({ name: 'Safe', mcVersion: '1.21.11', loader: 'fabric', color: '#aabbcc' }), {
    name: 'Safe', mcVersion: '1.21.11', loader: 'fabric', color: '#aabbcc',
  });
  assert.throws(() => profilePatch({ id: 'replace-me' }), /immutable/);
  assert.throws(() => profilePatch({ mcVersion: '../../etc' }), /Invalid/);
  assert.throws(() => profilePatch({ color: 'url(evil)' }), /Invalid/);
  assert.throws(() => profilePatch({ loader: 'custom' }), /Invalid/);
});
