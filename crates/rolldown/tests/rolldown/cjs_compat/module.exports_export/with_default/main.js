// Test that module.exports takes precedence over default export
const assert = require('node:assert');
const result = require('./esm.js');
assert.deepStrictEqual(result, { customExport: 'custom' });
