import assert from 'node:assert';

// `b.js` is in the gather list of `d.js`, so its error rejects the entry.
let error;
try {
  await import('./dist/main.js');
} catch (e) {
  error = e.message;
}
assert.strictEqual(error, 'b failed');
assert.deepStrictEqual(globalThis.deferLog, ['b rejects (import)', 'a']);
