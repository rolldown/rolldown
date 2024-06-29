import assert from 'node:assert'
assert.deepEqual(require('./cjs'), {
  foo: process
})
