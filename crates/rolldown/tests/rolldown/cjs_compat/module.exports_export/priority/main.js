// Test that module.exports takes priority over other exports
import assert from 'node:assert';
import result from './cjs.js';
assert.deepStrictEqual(result, { priorityValue: 'module.exports wins' });
