import assert from 'node:assert';
import mod from './cjs.js';

// `foo` appears in both `exports.foo = 1` and `module.exports = { foo: 2 }`.
// The constant must NOT be inlined from the stale `exports.foo = 1`.
assert.equal(mod.foo, 2);

// `bar` was written to the original exports object which got discarded by
// `module.exports = { foo: 2 }`. At runtime `mod.bar` is `undefined`.
// The stale constant must NOT be inlined either.
assert.equal(mod.bar, undefined);
