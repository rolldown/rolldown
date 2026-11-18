import assert from 'node:assert/strict';
await import('./dist/main.js');
assert.equal(globalThis.result, 1);
assert.equal(globalThis.styleLoaded, true);
