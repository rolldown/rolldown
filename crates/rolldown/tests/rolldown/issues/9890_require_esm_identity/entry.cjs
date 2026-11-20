const assert = require('node:assert/strict');

const a = require('./esm.mjs');
const b = require('./esm.mjs');
assert.strictEqual(a, b);
