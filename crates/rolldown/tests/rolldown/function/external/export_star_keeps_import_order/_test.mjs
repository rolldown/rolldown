import assert from 'node:assert';

globalThis.order = [];
const main = await import('./dist/main.js');
assert.deepStrictEqual(globalThis.order, ['ext1', 'ext2']);
assert.strictEqual(main.fromExt1, 'ext1');
