import assert from 'node:assert/strict';

const index = await import('./dist/index.js');
const mod = await import('./dist/mod.js');

assert.deepEqual(Object.keys(index), ['foo']);
assert.deepEqual(Object.keys(mod), ['foo']);
assert.equal(index.foo(1), true);
assert.equal(mod.foo(2), false);
