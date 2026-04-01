import assert from 'node:assert';
import fs from 'node:fs';
import path from 'node:path';

const file = fs.readFileSync(path.resolve(import.meta.dirname, './dist/entry.js'), 'utf-8');

// Mixed-mode: should emit two separate __toESM bindings
assert.ok(
  file.includes('__toESM(foo, 1)'),
  'should have node-mode __toESM for ESM importer (sub1.mjs)',
);
assert.ok(
  /\bfoo = __toESM\(foo\);/.test(file),
  'should have non-node-mode __toESM for non-ESM importer (sub2.js)',
);
