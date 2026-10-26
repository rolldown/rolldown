import assert from 'node:assert/strict';

// `r.js` loads no runtime and starts the `import()` ring `p3.js` -> `p2.js` -> `p1.js` -> `p3.js`,
// so every ring entry and `logger.js` (needs `__exportAll` once its facade is merged) can run
// without `main.js`. The runtime must not be hosted in `shared.js`.
const { loadP3 } = await import('./dist/r.js');
const { loadP2 } = await loadP3();
const { loadP1 } = await loadP2();
const { lazyLog } = await loadP1();
assert.equal(await lazyLog(), 'log: p1');
assert.equal(globalThis.sharedEvaluated, undefined);

const main = await import('./dist/main.js');
const f = await import('./dist/f.js');
assert.deepEqual(main.keys, ['vE', 'wE']);
assert.equal(main.message, 'log: main');
assert.deepEqual(f.keys, ['vF', 'wF']);
assert.equal(globalThis.sharedEvaluated, 1);
