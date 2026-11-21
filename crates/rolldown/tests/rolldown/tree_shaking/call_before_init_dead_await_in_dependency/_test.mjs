import assert from 'node:assert/strict';

await import('./dist/main.js');
assert.deepEqual(globalThis.calls, ['a done']);
