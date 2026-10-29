import assert from 'node:assert/strict'
assert.deepEqual(require('./cjs'), {
  foo: process
})
