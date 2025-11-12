// Test fallback behavior: ESM without module.exports should get traditional CJS conversion
import assert from 'node:assert';
import * as result from './cjs.js';
assert.deepStrictEqual(result.__esModule, true);
assert.deepStrictEqual(result.foo, 'foo');
assert.deepStrictEqual(result.bar, 'bar');
assert.deepStrictEqual(result.default, { defaultValue: 'default' });
