import assert from 'node:assert/strict';
import lib from './lib.cjs';
import { value } from './shared.js';

assert.equal(lib.value, 42);
assert.equal(lib.shared, value);
assert.equal(lib.shared(), 43);
export default lib;
