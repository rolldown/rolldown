import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { readdirSync } from 'node:fs';

assert.deepEqual(
  readdirSync(new URL('./dist/', import.meta.url)).filter((file) => file.endsWith('.js')),
  ['entry.js'],
);
assert.equal(
  execFileSync(process.execPath, ['dist/entry.js'], {
    cwd: import.meta.dirname,
    encoding: 'utf8',
  }),
  'function\n',
);
