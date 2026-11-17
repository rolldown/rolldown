import assert from 'node:assert/strict';

await import('./dist/a.js');
assert.deepEqual(globalThis.tlaOrder, ['b', 'c', 'a']);
