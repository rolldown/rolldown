import assert from 'node:assert/strict';

// `chunk_import_source_order_kept_barrel_cycle`, but `app.js` reads `IconA` through a namespace
// import of the barrel. The output still gets `IconA` from the chunk of `a.js`, so that chunk must
// not come before the chunk of the barrel.
await import('./dist/app.js');
assert.deepEqual(globalThis.loadOrder, ['a', 'b']);
