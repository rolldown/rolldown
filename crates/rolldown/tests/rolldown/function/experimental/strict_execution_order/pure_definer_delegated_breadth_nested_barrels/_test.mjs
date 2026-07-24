import assert from 'node:assert';

globalThis.__events = [];

const a = await import('./dist/a.js');
await import('./dist/b.js');

// The consumer's namespace evidence lives on the OUTER pure barrel's star record, but that record's
// unrestricted walk stops at the init-owning MID barrel and delegates the rest of the chain. The
// breadth demand (every export the namespace retains, including `wDef` that no resolved read
// recorded) must survive that delegation, or the second pure definer is never initialized.
assert.deepStrictEqual(a.defValue, { value: 7 });
assert.deepStrictEqual(
  a.nsObject.wDef,
  { value: 11 },
  `delegated breadth must initialize the opaque-only pure definer; got ${JSON.stringify(a.nsObject.wDef)} events=${JSON.stringify(globalThis.__events)}`,
);
assert.deepStrictEqual(a.nsObject.vDef, { value: 7 });
