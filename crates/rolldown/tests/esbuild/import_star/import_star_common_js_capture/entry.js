import assert from 'node:assert'
import * as ns from './foo'
let foo = 234
assert.deepEqual(ns, {
  default: {
    foo: 123
  },
  foo: 123
})
assert.equal(ns.foo, 123)
assert.equal(foo, 234)
