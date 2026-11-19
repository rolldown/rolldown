import assert from 'node:assert/strict';

// The lowered `import()` prints `Promise.resolve()` inside the catch, so the linked catch `Promise`
// and its `var` must both be renamed, together.
async function run() {
  try {
    throw 0;
  } catch (Promise) {
    var Promise = 1;
    const mod = await import('./lazy.js');
    return [Promise, mod.value];
  }
}

run().then((result) => assert.deepStrictEqual(result, [1, 'lazy']));
