import assert from 'node:assert/strict';

globalThis.calls = [];
await assert.rejects(import('./dist/main.js'), {
  name: 'ReferenceError',
  message: 'window is not defined',
});
assert.deepEqual(globalThis.calls, []);
