'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const crypto = require('node:crypto');

test('scroll frames never expose Chromium white corner or resizer defaults', () => {
  const css = fs.readFileSync(path.resolve(__dirname, '../src/renderer/css/app.css'), 'utf8');
  assert.match(css, /::-webkit-scrollbar-corner,\s*\n::-webkit-resizer\s*\{\s*background:\s*transparent;/);
});

test('launcher UI loads as ordered design-system layers', () => {
  const html = fs.readFileSync(path.resolve(__dirname, '../src/renderer/index.html'), 'utf8');
  const expected = [
    'css/app.css',
    'css/tokens.css',
    'css/components.css',
    'css/layout.css',
    'css/home.css',
  ];
  let previous = -1;
  for (const href of expected) {
    const offset = html.indexOf(`href="${href}"`);
    assert.ok(offset > previous, `${href} should load after the preceding layer`);
    previous = offset;
  }
});

test('launcher tokens stay neutral and the new UI avoids glass effects', () => {
  const cssDir = path.resolve(__dirname, '../src/renderer/css');
  const tokens = fs.readFileSync(path.join(cssDir, 'tokens.css'), 'utf8');
  const allCss = fs.readdirSync(cssDir)
    .filter((file) => file.endsWith('.css'))
    .map((file) => fs.readFileSync(path.join(cssDir, file), 'utf8'))
    .join('\n');
  const modern = [
    'tokens.css',
    'components.css',
    'layout.css',
    'home.css',
  ].map((file) => fs.readFileSync(path.join(cssDir, file), 'utf8')).join('\n');

  assert.match(tokens, /--ink-0:\s*#07080a/);
  assert.match(tokens, /--mango:\s*#ffad32/);
  assert.match(tokens, /--space-1:\s*4px/);
  assert.doesNotMatch(modern, /backdrop-filter|filter:\s*blur/i);
  assert.doesNotMatch(allCss, /(?:linear|radial)-gradient\(/i);
});

test('home launch control is split and rail instances only activate in instance view', () => {
  const html = fs.readFileSync(path.resolve(__dirname, '../src/renderer/index.html'), 'utf8');
  const app = fs.readFileSync(path.resolve(__dirname, '../src/renderer/js/app.js'), 'utf8');

  assert.match(html, /id="btn-play"/);
  assert.match(html, /id="btn-home-profile"[^>]+aria-haspopup="menu"/s);
  assert.match(html, /id="home-profile-menu"[^>]+role="menu"/s);
  assert.match(app, /const instanceViewActive = document\.body\.dataset\.activeView === 'instance'/);
  assert.match(app, /instanceViewActive && p\.id === state\.profile\?\.id/);
});

test('mods and stats keep equal neutral modules with collision-safe spacing', () => {
  const html = fs.readFileSync(path.resolve(__dirname, '../src/renderer/index.html'), 'utf8');
  const appCss = fs.readFileSync(path.resolve(__dirname, '../src/renderer/css/app.css'), 'utf8');
  const componentCss = fs.readFileSync(path.resolve(__dirname, '../src/renderer/css/components.css'), 'utf8');
  const i18n = fs.readFileSync(path.resolve(__dirname, '../src/renderer/js/i18n.js'), 'utf8');

  assert.match(html, /data-i18n="home\.partneredServers"/);
  assert.match(i18n, /'home\.partneredServers': 'Partner-Server'/);
  assert.match(i18n, /'home\.partneredServers': 'Partnered Servers'/);
  assert.match(appCss, /\.card-grid\.mods\s*\{[^}]*grid-auto-rows:\s*1fr/s);
  assert.match(componentCss, /\.input-icon input\[type="search"\]\s*\{\s*padding-left:\s*38px/);
  assert.match(appCss, /#view-stats \.panel \+ \.panel\s*\{\s*margin-top:\s*16px/);
  assert.match(
    fs.readFileSync(path.resolve(__dirname, '../src/renderer/css/home.css'), 'utf8'),
    /\.home-profile-trigger\.btn\.brand\s*\{\s*border-left-color:/,
  );
});

test('rail and server hover actions escape clipping and stay keyboard reachable', () => {
  const html = fs.readFileSync(path.resolve(__dirname, '../src/renderer/index.html'), 'utf8');
  const app = fs.readFileSync(path.resolve(__dirname, '../src/renderer/js/app.js'), 'utf8');
  const home = fs.readFileSync(path.resolve(__dirname, '../src/renderer/css/home.css'), 'utf8');

  assert.match(html, /id="rail-instance-tooltip"[^>]+role="tooltip"/);
  assert.match(app, /btn\.onpointerenter = \(\) => showRailInstanceTooltip\(btn\)/);
  assert.match(app, /class="server-play-overlay"/);
  assert.match(home, /\.server-row:focus-visible \.server-play-overlay/);
  assert.match(html, /data-days="14">14d<\/button>/);
  assert.match(html, /data-days="30">30d<\/button>/);
  assert.match(html, /data-days="90">90d<\/button>/);
});

test('offline fallback is Mojang official 64x64 Steve texture', () => {
  const { defaultSkinDataUrl } = require('../src/main/defaultSkin');
  const png = Buffer.from(defaultSkinDataUrl().split(',')[1], 'base64');

  assert.equal(png.readUInt32BE(16), 64);
  assert.equal(png.readUInt32BE(20), 64);
  assert.equal(
    crypto.createHash('sha256').update(png).digest('hex'),
    '4c7b0468044bfecacc43d00a3a69335a834b73937688292c20d3988cae58248d',
  );
});

test('statistics exposes a time axis and MangoConfig uses one check state', () => {
  const html = fs.readFileSync(path.resolve(__dirname, '../src/renderer/index.html'), 'utf8');
  const app = fs.readFileSync(path.resolve(__dirname, '../src/renderer/js/app.js'), 'utf8');
  const css = fs.readFileSync(path.resolve(__dirname, '../src/renderer/css/app.css'), 'utf8');

  assert.doesNotMatch(html, /id="mc-pill"[\s\S]*?<span class="dot"/);
  assert.match(app, /\$\('#mc-state'\)\.innerHTML = info\.enabled && info\.supported \? icon\('check'\) : ''/);
  assert.match(app, /class="chart-y-axis"/);
  assert.match(app, /class="chart-grid-lines"/);
  assert.match(app, /tabindex="0" aria-label=/);
  assert.match(css, /\.chart-col:focus-visible::after\s*\{\s*display:\s*block/);
});
