import assert from 'node:assert';

// The preparation ignores the error of the started module `m1.js`. Only the access throws.
await import('./dist/main.js');
await new Promise((resolve) => setTimeout(resolve, 10));
assert.deepStrictEqual(globalThis.deferLog, [
  'a start',
  'a end',
  'm2',
  'import(m1) rejects',
  'i reads d: m1 failed',
]);
