import fs from 'node:fs';
import assert from 'node:assert';
import path from 'node:path';

const mapPath = path.resolve(import.meta.dirname, 'dist/main.js.map');
const sourceMap = JSON.parse(fs.readFileSync(mapPath, 'utf8'));

assert.ok(Array.isArray(sourceMap.names), 'names must be an array');
assert.ok(
  sourceMap.names.includes('greet'),
  `names should contain 'greet', got: ${JSON.stringify(sourceMap.names)}`,
);
assert.ok(
  sourceMap.names.includes('personName'),
  `names should contain 'personName', got: ${JSON.stringify(sourceMap.names)}`,
);
