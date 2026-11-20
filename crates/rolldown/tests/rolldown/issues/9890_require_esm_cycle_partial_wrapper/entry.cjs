const assert = require('node:assert/strict');

const ns = require('./esm.mjs');
assert.strictEqual(ns.late, 42);
assert.notStrictEqual(ns, require('./esm.mjs'));

const closed = require('./closed.mjs');
assert.strictEqual(closed.value, 42);
assert.strictEqual(closed.during, closed);
