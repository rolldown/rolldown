import assert from 'node:assert/strict';

globalThis.values = [];
await import('./dist/entries/a.js');
await import('./dist/entries/b.js');

assert.deepEqual(globalThis.values, ['a:42', 'b:42']);
