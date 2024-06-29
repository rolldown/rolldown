import assert from 'node:assert'
import * as ns from './foo'
assert.deepEqual(ns, {
  default: {
    x: 123
  },
  x: 123
})
assert.equal(ns.foo, undefined)
