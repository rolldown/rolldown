const assert = require('node:assert/strict');
const b = require('./dist/b.cjs');
assert.deepEqual(Object.keys(b), ['x']);
assert.equal(globalThis.y, 2);
require('./dist/e.cjs');
