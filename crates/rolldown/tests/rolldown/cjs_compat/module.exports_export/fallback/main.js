// Test fallback behavior: ESM without module.exports should get traditional CJS conversion
const assert = require('node:assert');
const result = require('./esm.js');
assert.deepStrictEqual(result.__esModule, true);
assert.deepStrictEqual(result.foo, 'foo');
assert.deepStrictEqual(result.bar, 'bar');
assert.deepStrictEqual(result.default, { defaultValue: 'default' });
