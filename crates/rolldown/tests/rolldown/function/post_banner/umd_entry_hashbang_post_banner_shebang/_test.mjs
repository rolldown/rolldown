import assert from 'node:assert';
import fs from 'node:fs';
import path from 'node:path';

const output = fs.readFileSync(path.resolve(import.meta.dirname, 'dist/main.js'), 'utf-8');

assert(
  output.startsWith('#!/usr/bin/env node\n(function'),
  `Expected output to start with only the postBanner shebang, but got:\n${output.slice(0, 100)}`,
);
assert.strictEqual(
  [...output.matchAll(/^#!/gm)].length,
  1,
  'UMD output should only contain the postBanner shebang',
);
