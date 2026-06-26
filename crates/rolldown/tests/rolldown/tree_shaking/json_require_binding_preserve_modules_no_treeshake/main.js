import assert from 'node:assert/strict';

const data = require('./data.json');
assert.strictEqual(data.foo, 'kept');
