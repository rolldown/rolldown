import assert from 'node:assert/strict';

globalThis.calls = [];
await import('./dist/entry.js');
assert.deepEqual(globalThis.calls, ['eval', 'redeclared', 'rest default', 'destructuring']);
