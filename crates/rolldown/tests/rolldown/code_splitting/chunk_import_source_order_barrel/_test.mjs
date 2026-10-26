import assert from 'node:assert/strict';

// `b` imports `value` through the `barrel` package, which has no side effects, so tree shaking
// removes it. The output imports `value` from the chunk of `read.js`, so that chunk must take the
// position of the `barrel` import, after the chunk of `init.js`.
const { result } = await import('./dist/b.js');
assert.equal(result, 'ready');
