import assert from 'node:assert';
import { readdirSync } from 'node:fs';
import { load } from './dist/main.js';

// `seg.js` gets no facade (see `mod.rs`).
const files = readdirSync(new URL('./dist', import.meta.url));
assert.deepStrictEqual(
  files.filter((file) => file.startsWith('seg')),
  [],
);

// `import()` gives the namespace object of the entry from the `group` chunk. The getter of `first`
// returns the inlined constant.
const ns = await load();
assert.deepStrictEqual(Object.keys(ns).sort(), ['first', 'foreign']);
assert.strictEqual(ns.first, 1);
assert.strictEqual(ns.foreign, 5);
