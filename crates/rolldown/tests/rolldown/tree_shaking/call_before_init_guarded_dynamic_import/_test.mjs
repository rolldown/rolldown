import assert from 'node:assert/strict';

globalThis.calls = [];
await import('./dist/main.js');
assert.deepEqual(globalThis.calls, ['same module: ReferenceError', 'a done', 'main done']);
