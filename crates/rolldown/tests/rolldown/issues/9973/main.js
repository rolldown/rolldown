import assert from 'node:assert';
// Importing the multi-declarator export by name fails to build (MISSING_EXPORT)
// if the `export` keyword is dropped (#9973).
import { a, b, c } from './dep.js';

assert.strictEqual(a, 1);
assert.strictEqual(b, 2);
assert.strictEqual(c, 3);
