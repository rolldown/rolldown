import assert from 'node:assert';
import { load } from './dist/main.js';
import * as seg from './dist/seg.js';

// The facade reads `n` when it loads.
assert.strictEqual(seg.first, 1);

// `import()` gives the namespace object of the entry from the `group` chunk. The getter of `first`
// returns the inlined constant.
const ns = await load();
assert.deepStrictEqual(Object.keys(ns).sort(), ['first', 'foreign']);
assert.strictEqual(ns.first, 1);
assert.strictEqual(ns.foreign, 5);
