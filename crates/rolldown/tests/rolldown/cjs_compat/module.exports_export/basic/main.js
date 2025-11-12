// Test basic module.exports export behavior
const assert = require('node:assert');
const esm = require('./esm.js');
assert.deepStrictEqual(esm, { foo: 'foo' });
