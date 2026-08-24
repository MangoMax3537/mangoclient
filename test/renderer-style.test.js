'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');

test('scroll frames never expose Chromium white corner or resizer defaults', () => {
  const css = fs.readFileSync(path.resolve(__dirname, '../src/renderer/css/app.css'), 'utf8');
  assert.match(css, /::-webkit-scrollbar-corner,\s*\n::-webkit-resizer\s*\{\s*background:\s*transparent;/);
});
