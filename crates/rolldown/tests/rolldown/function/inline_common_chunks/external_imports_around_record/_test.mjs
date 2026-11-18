import assert from 'node:assert/strict';

await import('./dist/a.js');
// The modules run when they are initialized, so only the external modules show the import order:
// `ext3`, which `big.js` imports, runs where `a.js` imports `shared.js`.
assert.deepEqual(
  globalThis.order.filter((name) => name.startsWith('ext')),
  ['ext1', 'ext3', 'ext2'],
);
