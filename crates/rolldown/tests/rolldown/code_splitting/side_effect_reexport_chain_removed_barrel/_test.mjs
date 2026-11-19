import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';

assert.equal(
  execFileSync(process.execPath, ['dist/entry.js'], { cwd: import.meta.dirname, encoding: 'utf8' }),
  'setup\nforward ready\nentry 2 1\n',
);
