import assert from 'node:assert/strict';
import {
  result,
  objectResult,
  arrayResult,
  bigintResult,
  compoundResult,
  cachedResult,
} from './dist/main.js';

assert.equal(result, '/foo/gi');
assert.equal(objectResult, 'custom object');
assert.equal(arrayResult, 'custom array');
assert.equal(bigintResult, '255');
assert.equal(compoundResult, '/foo/gi');
assert.equal(cachedResult, '/foo/gi');
