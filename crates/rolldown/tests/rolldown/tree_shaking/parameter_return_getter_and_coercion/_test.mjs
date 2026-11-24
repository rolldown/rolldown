import assert from 'node:assert/strict';

globalThis.calls = [];
await import('./dist/main.js');
assert.deepEqual(globalThis.calls, [
  'computed key',
  'getter',
  'valueOf',
  'toString',
  'getter',
  'toString',
  'this',
  'arguments',
  'argument evaluation',
]);
