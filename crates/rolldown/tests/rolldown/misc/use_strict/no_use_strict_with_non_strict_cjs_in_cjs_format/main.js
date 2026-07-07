import assert from 'node:assert/strict';
import foo from './cjs';
assert.deepEqual(foo, {
  default: {},
});

export {};
