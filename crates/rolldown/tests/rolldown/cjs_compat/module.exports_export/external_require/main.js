// Test that require('./external.mjs') works correctly when the ESM module exports 'module.exports'
// The optimization should skip __toCommonJS and directly access external_exports["module.exports"]
import assert from 'node:assert';
const external = require('./external.mjs');
assert.deepStrictEqual(external, { value: 'external module' });
