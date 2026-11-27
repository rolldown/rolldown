import assert from 'node:assert/strict';

// The entry chunk `a.js` is a facade of `a-impl`. Merging `s.js` into it would make `a-impl`
// import `s` back from the facade, and read it before it is initialized.
globalThis.log = [];
await import('./dist/a.js');
assert.deepStrictEqual(globalThis.log, ['a:s']);
