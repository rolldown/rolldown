import assert from 'node:assert/strict';

export async function foo() {
  return [await import('./exist-dep-cjs'), await import('./exist-dep-esm')];
}

export async function bar() {
  return [await import('./new-dep-cjs.js'), await import('./new-dep-esm.js')];
}

// The re-run calls `import()` of modules this client never received: `exist-dep-*` sit in
// chunks it never loaded, and `new-dep-*` are new. The calls run one after the other so each
// pull sees the factories the previous one delivered, which keeps the snapshot stable.
(async () => [await foo(), await bar()])().then(([[existCjs, existEsm], [newCjs, newEsm]]) => {
  assert.strictEqual(existCjs.default.value, 'exist-cjs');
  assert.strictEqual(existEsm.value, 'exist-esm');
  assert.strictEqual(newCjs.default.value, 'new-cjs');
  assert.strictEqual(newEsm.value, 'new-esm');
  globalThis.__dynamic_import_checked = true;
});
process.on('beforeExit', () => assert.ok(globalThis.__dynamic_import_checked));

import.meta.hot.accept((mod) => {
  if (mod) {
    console.log('.hmr', mod.foo);
  }
});
