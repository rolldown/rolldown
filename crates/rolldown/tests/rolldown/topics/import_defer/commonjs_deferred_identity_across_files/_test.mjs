import assert from 'node:assert/strict';

globalThis.evaluations = 0;
const { a, b } = await import('./dist/main.js');
const keys = new WeakMap([[a, 'shared']]);
assert.equal(keys.get(b), 'shared');
assert.equal(globalThis.evaluations, 0);
assert.equal(a.default.value, 1);
assert.equal(b.default.value, 1);
assert.equal(globalThis.evaluations, 1);
