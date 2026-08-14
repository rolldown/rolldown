import assert from 'node:assert';
import * as cjs from './cjs.cjs';
import * as cjs2 from './cjs2.cjs';

// cjs.default = module.exports (node ESM mode, __esModule ignored)
// cjs.default.default = module.exports.default = exports.default
assert.deepStrictEqual(cjs.default, { default: 'default' });
assert.deepStrictEqual(cjs.default.default, 'default');

assert.deepStrictEqual(cjs2.default, 'default');
