import assert from 'node:assert/strict';

await import('./dist/a.js');
assert.deepEqual(globalThis.externalOrder, ['ext', 'shared']);
assert.deepEqual(globalThis.values, ['ext', 'shared']);
