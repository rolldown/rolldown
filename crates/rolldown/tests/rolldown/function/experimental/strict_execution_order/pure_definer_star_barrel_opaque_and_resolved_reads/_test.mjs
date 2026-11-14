import assert from 'node:assert';

globalThis.__events = [];

const a = await import('./dist/a.js');
await import('./dist/b.js');

// The resolved read must observe the first definer.
assert.deepStrictEqual(a.defValue, { value: 7 });
// The opaque namespace route must observe the second definer: a retained path recorded for the
// resolved read must not restrict the barrel's init walk below what the included namespace
// retains, or `wDef` would read back undefined.
assert.deepStrictEqual(
  a.nsObject.wDef,
  { value: 11 },
  `opaque namespace read must see the second pure definer; got ${JSON.stringify(a.nsObject.wDef)} events=${JSON.stringify(globalThis.__events)}`,
);
assert.deepStrictEqual(a.nsObject.vDef, { value: 7 });
