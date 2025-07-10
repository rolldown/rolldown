import assert from 'node:assert'
import _accepts from './cjs.js';

var accepts = typeof _accepts === "function" ? _accepts : _accepts.default;
assert.strictEqual(accepts(), 123);
