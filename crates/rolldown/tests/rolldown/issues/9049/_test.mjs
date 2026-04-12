import assert from 'node:assert';

// Regression test for #9049: verify that chunk optimization does not
// create circular static imports when merging common chunks.
// This complements #8361 which tests the runtime chunk case.
const { e1 } = await import('./dist/entry1.js');
assert.strictEqual(e1, 'shared12:baseshared123:basebase');

const { e2 } = await import('./dist/entry2.js');
assert.strictEqual(e2, 'shared12:baseshared123:basebase');

const { e3 } = await import('./dist/entry3.js');
assert.strictEqual(e3, 'shared123:basebase');

const { e4 } = await import('./dist/entry4.js');
assert.strictEqual(e4, 'base');
