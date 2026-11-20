import assert from 'node:assert/strict';

// `main.js` gets `name` and `SIZE` through the barrel `ui/index.js`, which has side effects. The
// barrel does not import its two owners, and neither owner imports the barrel, so the owners are
// not in a chunk cycle with it. The chunk of `main.js` must keep both owners before the barrel in
// the source order: `ui/o.js`, then `ui/button.js` with its style, then the barrel.
await import('./dist/main.js');
assert.deepEqual(globalThis.loadOrder, ['o', 'style', 'barrel', 'main md o']);
