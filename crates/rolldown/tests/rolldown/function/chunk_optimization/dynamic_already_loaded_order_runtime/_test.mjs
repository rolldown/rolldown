import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import path from 'node:path';

// Each entry runs in its own process: the shared chunks run once per process.
const run = (entry) =>
  execFileSync(
    process.execPath,
    [
      '--input-type=module',
      '-e',
      `globalThis.orderRuntimeLog = []; await import(${JSON.stringify(path.join(import.meta.dirname, 'dist', `${entry}.js`))}); console.log(JSON.stringify(globalThis.orderRuntimeLog));`,
    ],
    { encoding: 'utf8' },
  ).trim();

assert.equal(run('e0'), '["m3","m0","e0"]');
assert.equal(run('e2'), '["m0","m3"]');
