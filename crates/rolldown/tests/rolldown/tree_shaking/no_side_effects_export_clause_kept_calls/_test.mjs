import assert from 'node:assert/strict';

globalThis.calls = [];
await import('./dist/entry.js');
assert.deepEqual(globalThis.calls, [
  'eval',
  'redeclared',
  'rest default',
  'destructuring',
  'coercion',
  'getter',
  'getter',
  'ReferenceError',
]);
await assert.rejects(import('./dist/early.js'), ReferenceError);
