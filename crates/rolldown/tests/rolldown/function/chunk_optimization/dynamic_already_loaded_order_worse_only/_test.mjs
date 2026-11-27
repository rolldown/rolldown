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
      `globalThis.orderWorseLog = []; await import(${JSON.stringify(path.join(import.meta.dirname, 'dist', `${entry}.js`))}); console.log(JSON.stringify(globalThis.orderWorseLog));`,
    ],
    { encoding: 'utf8' },
  ).trim();

assert.equal(run('m'), '["a1","a2","a5"]');
assert.equal(run('e'), '["a1","a2","a5"]');
