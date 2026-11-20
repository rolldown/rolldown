import assert from 'node:assert/strict';

globalThis.threw = [];
await import('./dist/main.js');
assert.deepEqual(globalThis.threw, ['local', 'reexported']);
