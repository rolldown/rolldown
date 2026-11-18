import assert from 'node:assert';

const { order } = await import('./dist/a.js');
assert.deepEqual(order, ['m1', 'm6', 'm2', 'm0']);
