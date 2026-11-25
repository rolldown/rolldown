import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

assert.deepEqual(
  execFileSync(process.execPath, [fileURLToPath(new URL('./dist/main.js', import.meta.url))], {
    encoding: 'utf8',
    timeout: 10000,
  })
    .trim()
    .split(/\r?\n/),
  ['trigger', 'loaded object', 'target', 'value 1'],
);
