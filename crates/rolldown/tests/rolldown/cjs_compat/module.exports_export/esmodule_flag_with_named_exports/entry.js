// Test that module.exports.__esModule = true does not prevent tree-shaking of named exports
import assert from 'node:assert';
import { foo } from './cjs.js';

assert.strictEqual(foo, 'foo');
