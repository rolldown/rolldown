import assert from 'node:assert';

globalThis.events = [];
await import('./dist/b.js');

assert.deepStrictEqual(
  globalThis.events,
  ['S body', 'B body S'],
  'entry `e` shares a chunk with `shared.js`, so loading it for `b` must not run `e`',
);

await import('./dist/e.js');
assert.deepStrictEqual(
  globalThis.events,
  ['S body', 'B body S', 'E body'],
  'entry `e` runs its program when it is entered',
);
