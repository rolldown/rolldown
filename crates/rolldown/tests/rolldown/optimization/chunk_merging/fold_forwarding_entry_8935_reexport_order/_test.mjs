import assert from 'node:assert/strict';

const source = await import('./index.mjs');
const bundle = await import('./dist/index.mjs');
assert.deepEqual([source.foo(), source.captured], [1, 1]);
assert.deepEqual([bundle.foo(), bundle.captured], [source.foo(), source.captured]);
