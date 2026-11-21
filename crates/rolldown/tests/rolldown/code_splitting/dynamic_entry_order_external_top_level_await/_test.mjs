import assert from 'node:assert/strict';
await import('./dist/main.js');
assert.deepEqual(globalThis.log, ['m1 sees set']);
