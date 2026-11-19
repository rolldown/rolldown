import assert from 'node:assert/strict';

globalThis.calls = [];
await import('./dist/main.js');
assert.deepEqual(globalThis.calls, [
  'function',
  'arrow',
  'function expression',
  'object pattern',
  'default export',
  'function',
]);
