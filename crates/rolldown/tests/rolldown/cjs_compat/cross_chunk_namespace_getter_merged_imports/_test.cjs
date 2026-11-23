const assert = require('node:assert/strict');
assert.equal(require('./dist/a.cjs').a, 1);
assert.equal(require('./dist/b.cjs').b, 2);
