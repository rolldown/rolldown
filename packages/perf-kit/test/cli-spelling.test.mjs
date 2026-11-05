import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { test } from 'node:test';
import { fileURLToPath } from 'node:url';

const KIT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');

/** perf-kit copied under node_modules with the given manifest; returns the usage line `why` prints. */
function usageLineInstalledAs(manifest) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'perf-kit-cli-'));
  const dir = path.join(root, 'node_modules', manifest.name.replace('@', '').replace('/', '-'));
  fs.mkdirSync(dir, { recursive: true });
  for (const part of ['harness.mjs', 'lib']) {
    fs.cpSync(path.join(KIT, part), path.join(dir, part), { recursive: true });
  }
  fs.writeFileSync(path.join(dir, 'package.json'), JSON.stringify(manifest));
  const run = spawnSync(process.execPath, [path.join(dir, 'harness.mjs'), 'why'], {
    cwd: root,
    encoding: 'utf8',
    timeout: 20_000,
  });
  return (run.stdout + run.stderr).split('\n').find((l) => l.includes('usage:'));
}

test('an installed perf-kit spells hints as `npx <package name>` when a bin carries that name', () => {
  const line = usageLineInstalledAs({
    name: 'rolldown-perf-kit',
    bin: { 'rolldown-perf-kit': './harness.mjs', 'perf-kit': './harness.mjs' },
  });
  assert.match(line, /usage: npx rolldown-perf-kit why/);
});

test('a scoped package spells its full name when a bin carries the bare name', () => {
  const line = usageLineInstalledAs({
    name: '@rolldown/perf-kit',
    bin: { 'perf-kit': './harness.mjs', 'perf-kit-replay': './replay.mjs' },
  });
  assert.match(line, /usage: npx @rolldown\/perf-kit why/);
});

test('an installed perf-kit without a bin of its own name uses the first bin', () => {
  const line = usageLineInstalledAs({
    name: '@rolldown/perf-kit',
    bin: { kit: './harness.mjs', 'perf-kit-replay': './replay.mjs' },
  });
  assert.match(line, /usage: npx kit why/);
});

test('from the repo the hints name the script', () => {
  const run = spawnSync(process.execPath, [path.join(KIT, 'harness.mjs'), 'why'], {
    cwd: KIT,
    encoding: 'utf8',
    timeout: 20_000,
  });
  assert.match(run.stdout + run.stderr, /usage: node harness\.mjs why/);
});
