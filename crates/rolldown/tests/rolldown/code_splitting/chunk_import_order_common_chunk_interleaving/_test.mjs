import assert from 'node:assert/strict';

// A fresh process imports entry `b` only. `b.js` imports `m2.js` and then `m1.js`. `m2.js` loads
// `y.js`, which sets the global, and `m1.js` loads `x.js`, which reads it. The common chunk of
// `m1.js` and `m2.js` must import the chunk of `y.js` before the chunk of `x.js`.
const { value } = await import('./dist/b.js');
assert.equal(value, 'ready');
