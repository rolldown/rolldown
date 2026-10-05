import assert from 'node:assert/strict';

await import('./dist/main.js');
assert.deepEqual(globalThis.__result, [true, true, 'string']);
