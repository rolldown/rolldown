import assert from 'node:assert/strict';

const e1 = await import('./dist/e1.js');
assert.deepEqual(Object.keys(e1), ['default']);
assert.deepEqual(Object.keys(e1.default), []);
