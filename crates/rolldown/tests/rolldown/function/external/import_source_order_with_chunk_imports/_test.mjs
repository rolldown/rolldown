import assert from 'node:assert/strict';

await import('./dist/index.js');
assert.deepEqual(globalThis.externalOrder, ['polyfill', 'lib', 'app']);
