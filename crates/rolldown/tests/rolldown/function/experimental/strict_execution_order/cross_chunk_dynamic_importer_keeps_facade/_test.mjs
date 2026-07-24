import assert from 'node:assert';

// A dynamic entry with a cross-chunk dynamic importer keeps its facade chunk (asserted
// by the snapshot), so its trigger runs synchronously within the facade's module
// evaluation. Executing entry `b` must initialize `target` without triggering entry
// `a`'s side effect, and `target` must initialize exactly once across both entries.

globalThis.log = [];

const { bTargetPromise } = await import(new URL('./dist/b.js', import.meta.url));
const nsFromB = await bTargetPromise;
assert.strictEqual(nsFromB.value, 1);
assert.deepStrictEqual(globalThis.log, ['b', 'target']);

const { aTargetPromise } = await import(new URL('./dist/a.js', import.meta.url));
const nsFromA = await aTargetPromise;
assert.strictEqual(nsFromA.value, 1);
assert.deepStrictEqual(globalThis.log, ['b', 'target', 'a']);
