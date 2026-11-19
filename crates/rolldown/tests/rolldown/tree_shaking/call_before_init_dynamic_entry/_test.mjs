import assert from 'node:assert/strict';

globalThis.calls = [];
await import('./dist/main.js');
assert.deepEqual(globalThis.calls, [
  'declared: ReferenceError',
  'declared argument: ReferenceError',
  'clause: ReferenceError',
  'clause argument: ReferenceError',
]);
