import fs from 'node:fs';
import path from 'node:path';
import assert from 'node:assert';

const dist = path.join(import.meta.dirname, 'dist');
const files = fs
  .readdirSync(dist)
  .filter((f) => f !== 'package.json')
  .sort();

// tags: ['$initial'] should capture modules in the static import chain of the entry
// but NOT modules only reachable via dynamic import
assert.deepStrictEqual(files, ['initial-deps.js', 'lazy.js', 'main.js']);

const initialDeps = fs.readFileSync(path.join(dist, 'initial-deps.js'), 'utf-8');
assert.ok(
  initialDeps.includes('shared.js'),
  'initial-deps should contain shared.js (statically imported)',
);
assert.ok(
  !initialDeps.includes('lazy-dep.js'),
  'initial-deps should NOT contain lazy-dep.js (dynamic-only)',
);

const lazyChunk = fs.readFileSync(path.join(dist, 'lazy.js'), 'utf-8');
assert.ok(lazyChunk.includes('lazy-dep.js'), 'lazy chunk should contain lazy-dep.js');
assert.ok(!lazyChunk.includes('shared.js'), 'lazy chunk should NOT contain shared.js');
