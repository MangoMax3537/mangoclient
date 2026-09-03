'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');

const root = path.resolve(__dirname, '..');
const read = (file) => fs.readFileSync(path.join(root, file), 'utf8');

test('MangoConfig release metadata and patched org.json are current', () => {
  assert.match(read('mod/gradle.properties'), /^mod_version=1\.9\.5$/m);
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

test('ping is measured with the play-protocol round trip, not read off the tab list', () => {
  const ping = read('mod/src/client/java/gg/mangoclient/mangoconfig/Ping.java');
  const module = read('mod/src/client/java/gg/mangoclient/mangoconfig/modules/PingModule.java');
  const handler = read('mod/src/client/java/gg/mangoclient/mangoconfig/mixin/ClientPlayNetworkHandlerMixin.java');
  const tick = read('mod/src/client/java/gg/mangoclient/mangoconfig/mixin/MinecraftClientMixin.java');
  const mixins = read('mod/src/client/resources/mangoconfig.client.mixins.json');

  assert.match(ping, /new QueryPingC2SPacket\(now\)/);
  assert.match(ping, /if \(startTime != outstanding\) return/);
  assert.match(ping, /if \(\+\+missed >= GIVE_UP_AFTER\) supported = false/);
  assert.match(ping, /if \(handler != connection\)/);
  assert.match(handler, /method = "onPingResult"/);
  assert.match(handler, /Ping\.onResult\(packet\.startTime\(\)\)/);
  assert.match(tick, /Ping\.onClientTick\(mc\)/);
  assert.match(mixins, /"ClientPlayNetworkHandlerMixin"/);

  // The tab list is the fallback, so the measurement has to be read first.
  assert.ok(module.indexOf('Ping.latency()') < module.indexOf('getPlayerListEntry'));
  assert.match(module, /int measured = Ping\.latency\(\);\s*\n\s*if \(measured >= 0\) return measured;/);
});

test('the in-game palette is the launcher palette', () => {
  const css = read('src/renderer/css/app.css');
  const theme = read('mod/src/client/java/gg/mangoclient/mangoconfig/Theme.java');
  const variable = (name) => {
    const found = new RegExp(`--${name}:[ \t]*#([0-9a-f]{6})`, 'i').exec(css);
    assert.ok(found, `app.css is missing --${name}`);
    return found[1].toUpperCase();
  };
  const constant = (name) => {
    const found = new RegExp(`${name} = 0x([0-9A-F]{8})`).exec(theme);
    assert.ok(found, `Theme.java is missing ${name}`);
    return found[1];
  };

  for (const [css_, java] of [
    ['surface-1', 'SURFACE_1'], ['surface-2', 'SURFACE_2'],
    ['surface-3', 'SURFACE_3'], ['surface-4', 'SURFACE_4'],
    ['text', 'TEXT'], ['text-2', 'TEXT_2'], ['text-3', 'TEXT_3'],
    ['brand', 'BRAND'], ['brand-hover', 'BRAND_HOVER'],
    ['brand-deep', 'BRAND_DEEP'], ['brand-fg', 'BRAND_FG'],
    ['ok', 'OK'], ['warn', 'WARN'], ['danger', 'DANGER'],
  ]) {
    assert.equal(constant(java), `FF${variable(css_)}`, `${java} should be --${css_}`);
  }

  // The HUD plate is the window background at half opacity.
  assert.equal(constant('HUD_BG'), `80${variable('surface-1')}`);
  // And nothing in the HUD may hardcode a status colour behind Theme's back.
  for (const file of ['PingModule', 'MemoryModule', 'ComboModule', 'ArmourModule', 'PotionsModule']) {
    const source = read(`mod/src/client/java/gg/mangoclient/mangoconfig/modules/${file}.java`);
    assert.doesNotMatch(source, /0xFF(1BD96A|F2A53C|FF4B4B)/i, `${file} should use Theme`);
  }
});
