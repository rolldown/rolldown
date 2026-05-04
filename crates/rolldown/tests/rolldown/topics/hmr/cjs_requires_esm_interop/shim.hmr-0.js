import assert from 'node:assert';

const tags = require('./data.json');
assert.deepStrictEqual(tags, ['a', 'b', 'c']);

const fromEsm = require('./esm.js');
assert.strictEqual(fromEsm.X, 1);
assert.strictEqual(fromEsm.default, 'esm-default');
assert.strictEqual(fromEsm.__esModule, true);

globalThis.__cjs_requires_esm_interop_patch_ran = true;
