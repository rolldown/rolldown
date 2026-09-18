import assert from 'node:assert';

globalThis.events = [];
await import('./dist/a.js');
await import('./dist/b.js');

assert.deepStrictEqual(globalThis.events, [
  'true,true,true,true,true,true,true,true,true,true',
  'true',
]);
