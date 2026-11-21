import assert from 'node:assert/strict';

globalThis.calls = [];
await import('./dist/main.js');
assert.deepEqual(globalThis.calls, [
  'no side effects: ReferenceError',
  'let: ReferenceError',
  'empty arrow: ReferenceError',
  'namespace: ReferenceError',
  'same module: ReferenceError',
]);
await assert.rejects(import('./dist/bare.js'), ReferenceError);
