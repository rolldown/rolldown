import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';

const output = execFileSync(process.execPath, ['dist/main.js'], {
  cwd: import.meta.dirname,
  encoding: 'utf8',
});

assert.equal(output, 'root-src\n');
