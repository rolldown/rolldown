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
  ['a start', 'b start', 'a end', 'b end', 'target', 'main 1'],
);

const entry = JSON.stringify(new URL('./dist/main.js', import.meta.url).href);
const reject = `globalThis.rejectSibling = true; await import(${entry}).catch(e => console.log(e.message));`;
assert.deepEqual(
  execFileSync(process.execPath, ['--input-type=module', '-e', reject], {
    encoding: 'utf8',
    timeout: 10000,
  })
    .trim()
    .split(/\r?\n/),
  ['a start', 'b start', 'b failed'],
);
