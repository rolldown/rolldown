import assert from 'node:assert'
import * as ns from './foo'
import { bar, value, foo } from './foo'
assert.equal(bar, 1)
assert.equal(value, 1)
assert.equal(foo, undefined)
assert.equal(Object.keys(ns).length, 2)
export { bar } from './commonjs'
