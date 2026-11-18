import assert from 'node:assert/strict';

const { order } = await import('./dist/namespace.js');
assert.deepEqual(order, ['setup', 'owner']);
