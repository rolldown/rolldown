import assert from 'node:assert'
import * as ns from './common'
assert.deepEqual(ns, {
  [Symbol.toStringTag]: 'Module',
  x: 1,
  z: 4
})
