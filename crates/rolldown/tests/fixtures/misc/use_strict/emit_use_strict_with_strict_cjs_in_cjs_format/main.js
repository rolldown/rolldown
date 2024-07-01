import assert from 'assert'
import foo from './cjs'
assert.deepEqual(foo, {
  default: {}
})
export {}
