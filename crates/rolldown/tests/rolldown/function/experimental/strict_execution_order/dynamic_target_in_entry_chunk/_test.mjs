import assert from 'node:assert';

globalThis.__events = [];

// A manual group places entry `a` next to `b`, and `c` dynamically imports `b`. Loading that
// shared chunk must not run entry `a`'s program.
const c = await import('./dist/c.js');
await c.default.load();
assert.deepStrictEqual(
  globalThis.__events,
  ['b'],
  'loading dynamic target entry must not execute co-located entry a',
);

await import('./dist/a.js');
assert.deepStrictEqual(globalThis.__events, ['b', 'a']);
