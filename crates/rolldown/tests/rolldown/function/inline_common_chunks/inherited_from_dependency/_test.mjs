import assert from 'node:assert';

globalThis.events = [];
await import('./dist/a.js');
await import('./dist/b.js');

assert.deepStrictEqual(globalThis.events, ['S body', 'V body', 'A 1 2 2', 'B 3']);
