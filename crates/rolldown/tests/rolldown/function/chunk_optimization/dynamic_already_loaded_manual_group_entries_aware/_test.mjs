import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';

const dist = path.join(import.meta.dirname, 'dist');
const mainCode = fs.readFileSync(path.join(dist, 'main.js'), 'utf8');
const importedChunks = [...mainCode.matchAll(/^import .*"\.\/(.+)";$/gm)].map((match) => match[1]);
// A strict entry signature keeps the fold from adding exports to `main.js`, so nothing folds.
if (globalThis.__configName !== 'extended-preserve-entry-signatures-strict') {
  assert.equal(importedChunks.length, 1, importedChunks.join());
}

await import('./dist/main.js');
const { core, routes } = globalThis.entriesAwareMain;
assert.equal(core, 'c0,c1,c2,c3');
const values = await Promise.all(routes.map(async (load) => (await load()).value));
assert.deepEqual(values, ['c0c1', 'c1c2', 'c2c3', 'c3c0']);
