import assert from 'node:assert/strict';

globalThis.calls = [];
await import('./dist/main.js');
assert.deepEqual(globalThis.calls, ['define: ReferenceError', 'coalesce: ReferenceError']);
