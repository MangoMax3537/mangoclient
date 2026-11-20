#!/usr/bin/env node
// Asserts that bundle/mangoconfig holds exactly the 14 MangoConfig jars for the
// mod_version in mod/gradle.properties (names must not change, see CLAUDE.md).
import { readFileSync, readdirSync, statSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const props = readFileSync(join(root, 'mod/gradle.properties'), 'utf8');
const version = /^mod_version\s*=\s*(\S+)\s*$/m.exec(props)?.[1];
if (!version) {
  console.error('mod_version not found in mod/gradle.properties');
  process.exit(1);
}

const mcVersions = ['1.20.5', '1.20.6', '1.21'];
for (let i = 1; i <= 11; i++) mcVersions.push(`1.21.${i}`);

const dir = join(root, 'bundle/mangoconfig');
let present;
try {
  present = new Set(readdirSync(dir));
} catch {
  console.error(`missing directory ${dir}`);
  process.exit(1);
}

const expected = mcVersions.map((mc) => `MangoConfig-${version}+mc${mc}.jar`);
const problems = [];
for (const name of expected) {
  if (!present.has(name)) problems.push(`missing: ${name}`);
  else if (statSync(join(dir, name)).size < 1024) problems.push(`suspiciously small: ${name}`);
}
for (const name of present) {
  if (/^MangoConfig-.*\.jar$/.test(name) && !expected.includes(name)) {
    problems.push(`stale or unexpected jar (two jars with one id crash Fabric): ${name}`);
  }
}
if (problems.length) {
  console.error(`MangoConfig jars do not match mod_version ${version}:`);
  for (const p of problems) console.error(`  ${p}`);
  process.exit(1);
}
console.log(`OK: ${expected.length} MangoConfig jars for mod_version ${version}`);
