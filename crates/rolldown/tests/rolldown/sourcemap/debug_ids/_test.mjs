import fs from 'node:fs';
import assert from 'node:assert';
import path from 'node:path';

const source = fs.readFileSync(path.resolve(import.meta.dirname, 'dist/assets/main.js'), 'utf8');
const match = source.match(/\/\/# debugId=([a-fA-F0-9-]+)/);

assert.ok(match, 'Could not find debugId in source');
const sourceDebugId = match[1];

const sourceMap = JSON.parse(
  fs.readFileSync(path.resolve(import.meta.dirname, 'dist/assets/main.js.map'), 'utf8'),
);
assert.equal(sourceMap.debugId, sourceDebugId, 'debugId mismatch');
