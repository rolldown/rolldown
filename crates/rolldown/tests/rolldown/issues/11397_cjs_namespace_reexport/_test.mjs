import assert from 'node:assert/strict';

// `target.mjs` exports the namespace of a CommonJS module. A signature facade for it made the CJS
// build panic.
await import('./dist/main.js');
assert.equal(await globalThis.run(), 1);
assert.equal(globalThis.before, 0);
assert.equal(globalThis.saved.ns.default.n, 8);
