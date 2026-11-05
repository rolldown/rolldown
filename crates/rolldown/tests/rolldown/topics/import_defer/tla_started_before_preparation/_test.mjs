import assert from 'node:assert';

await import('./dist/main.js');
await new Promise((resolve) => setTimeout(resolve, 10));
// The preparation waits for `a.js`, then for the started modules `m1.js` and `m2.js`. So the access works.
assert.deepStrictEqual(globalThis.deferLog, ['a start', 'a end', 'm2', 'm1', 'd', 'i reads d: ok']);
