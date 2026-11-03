import assert from 'node:assert';

globalThis.events = [];
await import('./dist/a.js');
await import('./dist/b.js');

assert.deepStrictEqual(globalThis.events, [
  'A undefined/undefined 1 entry-a',
  'B undefined/undefined 1',
]);
