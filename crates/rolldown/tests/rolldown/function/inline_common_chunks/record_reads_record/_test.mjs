import assert from 'node:assert';

globalThis.events = [];
await import('./dist/c.js');
await import('./dist/a.js');
await import('./dist/b.js');

assert.deepStrictEqual(globalThis.events, [
  'S3',
  'C 3 Uc',
  'S1',
  'A 4 Ut|4 c 3',
  'B 4 Ub|4 Ut|4 c',
]);
