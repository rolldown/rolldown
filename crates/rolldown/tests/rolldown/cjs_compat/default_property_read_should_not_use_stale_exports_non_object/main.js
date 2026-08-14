import assert from 'node:assert';
import mod from './cjs.js';

// module.exports is assigned a non-object-literal (function call), so
// all exports.xxx constants must be invalidated — can't know which
// properties the result will have.
assert.equal(mod.foo, 2);
