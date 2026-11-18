import assert from 'node:assert';

globalThis.events = [];
await import('./dist/a.js');
await import('./dist/b.js');
await import('./dist/c.js');

assert.deepStrictEqual(globalThis.events, ['A ab1 ac2 a3 3', 'B ab1', 'C ac2']);
