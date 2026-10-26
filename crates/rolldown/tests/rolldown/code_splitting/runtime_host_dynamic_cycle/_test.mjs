import assert from 'node:assert/strict';
import fs from 'node:fs';

// `b.js` and `c.js` import each other through `import()`, so they are not in topological order.
// `b.js` still loads only after `main.js`, so the runtime is hosted in `shared.js`.
assert.deepEqual(
  fs.readdirSync(new URL('./dist', import.meta.url)).filter((file) => file.startsWith('rolldown-runtime')),
  [],
);

const main = await import('./dist/main.js');
const f = await import('./dist/f.js');
assert.deepEqual(main.keys, ['vE', 'wE']);
assert.deepEqual(f.keys, ['vF', 'wF']);
const c = await main.loadC();
const b = await c.loadB();
assert.deepEqual(b.keys, ['vB', 'wB']);
assert.equal(globalThis.sharedEvaluated, 1);
