import assert from 'node:assert/strict';

// The source does not throw when only e2.js loads.
globalThis.calls = [];
await import('./dist/e2.js');
assert.deepEqual(globalThis.calls, []);
