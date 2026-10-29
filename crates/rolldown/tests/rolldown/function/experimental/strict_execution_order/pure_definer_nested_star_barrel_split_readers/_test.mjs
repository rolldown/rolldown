import assert from 'node:assert';

globalThis.__events = [];

const a = await import('./dist/a.js');
const b = await import('./dist/b.js');

// The outer barrel's retained path stops at the non-transparent inner barrel and delegates the
// remaining star hop to it; the inner barrel's `init_*` must therefore call `init_definer` before
// any namespace reader observes the definer's binding.
assert.deepStrictEqual(
  a.defValue,
  { value: 7 },
  `inner barrel init must initialize the pure definer; got ${JSON.stringify(a.defValue)} events=${JSON.stringify(globalThis.__events)}`,
);
assert.strictEqual(b.loaded, true);
