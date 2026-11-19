import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';

assert.equal(
  execFileSync(process.execPath, ['dist/entry.js'], {
    cwd: import.meta.dirname,
    encoding: 'utf8',
  }),
  '1\n',
);
