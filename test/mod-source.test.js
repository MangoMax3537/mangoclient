'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');

const root = path.resolve(__dirname, '..');
const read = (file) => fs.readFileSync(path.join(root, file), 'utf8');

test('MangoConfig release metadata and patched org.json are current', () => {
  assert.match(read('mod/gradle.properties'), /^mod_version=1\.9\.2$/m);
  assert.match(read('mod/build.gradle'), /org\.json:json:20251224/);
  assert.doesNotMatch(read('mod/build.gradle'), /org\.json:json:20230227/);
});

test('fullbright persists independently and restores raw gamma after lightmap updates', () => {
  const config = read('mod/src/client/java/gg/mangoclient/mangoconfig/ConfigFile.java');
  const mixin = read('mod/src/client/java/gg/mangoclient/mangoconfig/mixin/LightmapTextureManagerMixin.java');
  assert.match(config, /public boolean fullbright = false/);
  assert.match(config, /addProperty\("fullbright", fullbright\)/);
  assert.match(mixin, /mangoconfig\$setRawValue\(1500\.0D\)/);
  assert.match(mixin, /mangoconfig\$setRawValue\(mangoconfig\$normalGamma\)/);
});

test('empty armor is hidden in game while preview-aware bounds remain available', () => {
  const armor = read('mod/src/client/java/gg/mangoclient/mangoconfig/modules/ArmourModule.java');
  const mango = read('mod/src/client/java/gg/mangoclient/mangoconfig/MangoConfig.java');
  assert.match(armor, /boolean visible\(MinecraftClient mc\)/);
  assert.match(armor, /!hideEmpty\.value \|\| !stacks\(mc, false\)\.isEmpty\(\)/);
  assert.match(mango, /module\.width\(mc, font, preview\)/);
  assert.match(mango, /module\.height\(mc, font, preview\)/);
});

test('ping rejects the zero placeholder and falls back to the server-list measurement', () => {
  const ping = read('mod/src/client/java/gg/mangoclient/mangoconfig/modules/PingModule.java');
  assert.match(ping, /if \(tabLatency > 0\) return tabLatency/);
  assert.match(ping, /server\.ping > 0/);
  assert.match(ping, /return tabLatency == 0 \? -1 : tabLatency/);
});

test('zoom wheel input is consumed and the rendered factor is eased', () => {
  const mouse = read('mod/src/client/java/gg/mangoclient/mangoconfig/mixin/MouseMixin.java');
  const mods = read('mod/src/client/java/gg/mangoclient/mangoconfig/Mods.java');
  assert.match(mouse, /if \(!Mods\.zoomActive\) return/);
  assert.match(mouse, /Mods\.zoomLevel\.set/);
  assert.match(mouse, /ci\.cancel\(\)/);
  assert.match(mods, /Math\.exp\(-12\.0f \* seconds\)/);
  assert.match(mods, /float target = active \? zoomLevel\.get\(\) : 1\.0f/);

  for (const family of ['a', 'b', 'c']) {
    const renderer = read(`mod/src/hud/${family}/java/gg/mangoclient/mangoconfig/mixin/GameRendererMixin.java`);
    assert.match(renderer, /float factor = Mods\.zoomFactor\(\)/);
    assert.doesNotMatch(renderer, /if \(!Mods\.zoomActive\) return/);
  }
});
