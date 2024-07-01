import assert from 'node:assert'
import * as ns from './foo'
let foo = 234
assert.deepEqual(ns, {
  foo: 123
})
assert.equal(ns.foo, 123)
assert.equal(foo, 234)
