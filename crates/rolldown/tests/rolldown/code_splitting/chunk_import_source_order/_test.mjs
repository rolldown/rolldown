import assert from 'node:assert/strict';

// `a` imports `read.js` first, so `read.js` gets the earlier `exec_order`. `b` imports
// `init.js` first, and the chunk imports of `b` must keep that order.
const { value } = await import('./dist/b.js');
assert.equal(value, 'ready');
