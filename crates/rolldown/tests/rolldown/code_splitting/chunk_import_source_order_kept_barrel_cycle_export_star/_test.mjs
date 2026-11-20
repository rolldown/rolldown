import assert from 'node:assert/strict';

// `chunk_import_source_order_kept_barrel_cycle`, but `app.js` re-exports the barrel with
// `export *`. The output gets each binding from the chunk of the module that declares it, so these
// chunks must not come before the chunk of the barrel.
await import('./dist/app.js');
assert.deepEqual(globalThis.loadOrder, ['a', 'b']);
