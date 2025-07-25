import assert from 'assert';
import mainResult from './dist/main.js';

// Test that the top-level return statement is properly handled
// In CommonJS, a top-level return exits early but doesn't set exports
// Code after the return should not execute
assert.deepStrictEqual(mainResult, { before: "before return" });
assert.strictEqual(mainResult.after, undefined);