// Test that module.exports takes priority over other exports
const assert = require('node:assert');
const result = require('./esm.js');
assert.deepStrictEqual(result, { priorityValue: 'module.exports wins' });
