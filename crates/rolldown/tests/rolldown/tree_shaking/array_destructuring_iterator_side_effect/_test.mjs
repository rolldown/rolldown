import assert from 'node:assert/strict';

globalThis.calls = [];
await import('./dist/main.js');
assert.deepEqual(globalThis.calls, [
  'export declaration',
  'no side effects',
  'pure',
  'namespace',
  'imported',
  'local',
  'empty pattern',
  'hole',
  'rest element',
  'rest default',
]);
