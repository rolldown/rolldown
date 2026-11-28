import assert from 'node:assert';
import fs from 'node:fs';
import path from 'node:path';

// The deferred entry loads first. It never reads its namespace, so `x.cjs` does not run.
globalThis.__events = [];
await import('./dist/deferred.js');
assert.deepStrictEqual(globalThis.__events, []);
await import('./dist/eager.js');
assert.deepStrictEqual(globalThis.__events, ['x']);

// No code calls `__toESM`, so no chunk may contain it, also the chunk of `x.cjs`.
const dist = path.join(import.meta.dirname, 'dist');
for (const file of fs.readdirSync(dist).filter((name) => name.endsWith('.js'))) {
  const code = fs.readFileSync(path.join(dist, file), 'utf8');
  assert.ok(!code.includes('__toESM'), `${file} contains __toESM`);
}
