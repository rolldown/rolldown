import assert from 'node:assert';

// The walk from `d.js` goes through the started module `m1.js` to `a.js`. The error of `a.js` rejects the entry.
let error;
try {
  await import('./dist/main.js');
} catch (e) {
  error = e.message;
}
await new Promise((resolve) => setTimeout(resolve, 10));
assert.strictEqual(error, 'a failed');
assert.deepStrictEqual(globalThis.deferLog, ['a start', 'import(m1) rejects']);
