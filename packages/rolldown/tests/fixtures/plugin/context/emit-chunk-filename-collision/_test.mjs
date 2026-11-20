import assert from 'node:assert/strict';
import { right } from './dist/right3.js';
import { left } from './dist/shared.js';

assert.equal(right, 42);
assert.equal(left, 42);
