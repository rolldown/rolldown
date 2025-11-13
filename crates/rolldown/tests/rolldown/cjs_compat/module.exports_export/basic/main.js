// Test basic module.exports export behavior
import assert from 'node:assert';
import esm from './cjs.js';
assert.deepStrictEqual(esm, { foo: 'foo' });
