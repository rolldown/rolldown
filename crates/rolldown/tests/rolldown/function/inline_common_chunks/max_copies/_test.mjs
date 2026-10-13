import assert from 'node:assert';

globalThis.events = [];
await import('./dist/a.js');
await import('./dist/b.js');
await import('./dist/c.js');

assert.deepStrictEqual(globalThis.events, [
  'S body',
  'A 0 this:undefined',
  'B 1 this:undefined',
  'C 2 this:undefined',
]);
