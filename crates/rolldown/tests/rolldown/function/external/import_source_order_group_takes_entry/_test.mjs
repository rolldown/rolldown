import assert from 'node:assert/strict';

await import('./dist/b.js');
assert.deepEqual(globalThis.externalOrder, ['side-b', 'side-a']);
