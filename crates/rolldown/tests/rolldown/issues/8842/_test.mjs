import assert from 'node:assert';
import fs from 'node:fs';
import path from 'node:path';

const file = fs.readFileSync(path.resolve(import.meta.dirname, './dist/main.js'), 'utf-8');

assert.ok(
  file.includes('__toESM(jsonc_parser, 1)'),
  'should use node-mode __toESM for ESM importer',
);
