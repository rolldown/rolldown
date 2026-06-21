const a = require('./foo.js');
const b = require('./foo.js');

import assert from 'node:assert';
assert.strictEqual(a, b);
assert.strictEqual(a.foo, 123);
