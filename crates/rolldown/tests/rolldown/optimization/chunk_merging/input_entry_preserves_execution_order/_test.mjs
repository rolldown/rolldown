import assert from 'node:assert/strict';
await import('./dist/main.js');
assert.deepEqual(globalThis.order, ['a', 'base', 'main1']);
