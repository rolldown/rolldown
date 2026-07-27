import assert from 'node:assert';

// Neither dynamic importer needs a facade chunk (asserted by the snapshot): each
// import() rewrite carries the trigger itself. Executing entry `b` must still initialize
// `target` without triggering entry `a`'s side effect, and `target` must still initialize
// exactly once across both entries — the guarantee survives losing the file.

globalThis.log = [];

const { bTargetPromise } = await import(new URL('./dist/b.js', import.meta.url));
const nsFromB = await bTargetPromise;
assert.strictEqual(nsFromB.value, 1);
assert.deepStrictEqual(globalThis.log, ['b', 'target']);

const { aTargetPromise } = await import(new URL('./dist/a.js', import.meta.url));
const nsFromA = await aTargetPromise;
assert.strictEqual(nsFromA.value, 1);
assert.deepStrictEqual(globalThis.log, ['b', 'target', 'a']);
