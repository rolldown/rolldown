import assert from 'node:assert';

globalThis.__events = [];

const a = await import('./dist/a.js');
const b = await import('./dist/b.js');

// The pure definer is reached only through the barrel's `export *`. Under strict execution order
// the barrel's `init_*` must forward to `init_definer`; the regression emits `init_definer` with
// zero call sites, so `vDef` is never assigned and the namespace read observes `undefined`.
assert.deepStrictEqual(
  a.defValue,
  { value: 7 },
  `barrel init must initialize the pure definer; got ${JSON.stringify(a.defValue)} events=${JSON.stringify(globalThis.__events)}`,
);
assert.deepStrictEqual(b.sibValue, { value: 3 });
