import assert from 'node:assert'
import * as ns from './foo'
assert.equal(ns.foo, undefined)
assert.deepEqual(ns, {
  x: 123
})
