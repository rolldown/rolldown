import assert from 'node:assert/strict';

// `b.js` imports `a.js`, so the module of the entry `a` is not in its chunk yet when `s.js` is
// merged. Loading `c.js` must not run `a.js`.
globalThis.log = [];
const c = await import('./dist/c.js');
assert.strictEqual(await c.loaded, 'd:s');
assert.deepStrictEqual(globalThis.log, []);
