import assert from 'node:assert';
import fs from 'node:fs';

// The tree-shaken helper's record must not resurrect a facade file for the target.
assert.strictEqual(fs.existsSync(new URL('./dist/target.js', import.meta.url)), false);

const { targetPromise } = await import('./dist/a.js');
const target = await targetPromise;
assert.strictEqual(target.value, 1);

await import('./dist/b.js');
assert.deepStrictEqual(globalThis.log, ['a', 'target', 'used']);
