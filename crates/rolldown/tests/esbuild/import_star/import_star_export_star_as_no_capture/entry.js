import assert from 'node:assert'
import {ns} from './bar'
let foo = 234
console.log(ns.foo, ns.foo, foo)
assert.equal(ns.foo, 123)
assert(foo, 234)
