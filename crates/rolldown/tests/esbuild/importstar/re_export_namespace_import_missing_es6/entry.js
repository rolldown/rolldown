import assert from 'node:assert'
import {ns} from './foo'
assert.deepEqual(ns, {
  [Symbol.toStringTag]: 'Module',
  x: 123
})
assert.equal(ns.foo, undefined)
