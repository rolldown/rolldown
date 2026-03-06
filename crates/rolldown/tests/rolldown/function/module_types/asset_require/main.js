import assert from 'node:assert';
import importedAsset from './file.txt';
const requiredAsset = require('./file.txt');

// require() should return the string directly, not { __esModule, default }
assert.strictEqual(typeof requiredAsset, 'string');
// import should also work
assert.strictEqual(typeof importedAsset, 'string');
// Both should resolve to the same value
assert.strictEqual(importedAsset, requiredAsset);
