import assert from 'node:assert'
import foo from './cjs'
assert.deepEqual(foo, {
  default: {}
})

export {}
