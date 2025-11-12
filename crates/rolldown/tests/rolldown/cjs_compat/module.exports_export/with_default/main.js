// Test that module.exports takes precedence over default export
import assert from 'node:assert';
import result from './cjs.js';
assert.deepStrictEqual(result, { customExport: 'custom' });
