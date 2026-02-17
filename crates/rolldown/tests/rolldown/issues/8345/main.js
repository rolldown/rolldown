import assert from 'node:assert';

// This tests that conditional CJS re-exports preserve exports from all branches.
// When platform: "node", process.env.NODE_ENV is not defined, so both branches
// of the conditional require are included. The exports from both branches must
// be preserved (not tree-shaken).
const lib = require('./lib/index.js');
assert.strictEqual(typeof lib.a, 'number', 'lib.a should be a number');
assert.strictEqual(typeof lib.b, 'number', 'lib.b should be a number');
assert.strictEqual(lib.a, 1);
assert.strictEqual(lib.b, 2);
