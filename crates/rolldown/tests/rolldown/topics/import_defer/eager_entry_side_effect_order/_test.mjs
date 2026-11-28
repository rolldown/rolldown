import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

function run(entry) {
  const file = fileURLToPath(new URL(entry, import.meta.url));
  return execFileSync(process.execPath, [file], { encoding: 'utf8' }).trim().split(/\r?\n/);
}

// Each entry must see its own import order when it loads first.
assert.deepEqual(run('./dist/other.js'), ['a', 'last', 'other']);
assert.deepEqual(run('./dist/main.js'), ['last', 'main object']);
