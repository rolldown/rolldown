import assert from 'node:assert/strict';
import * as ns from './commonjs.js';
assert.deepEqual(ns, {
  default: {
    a: 1,
  },
  a: 1,
});
