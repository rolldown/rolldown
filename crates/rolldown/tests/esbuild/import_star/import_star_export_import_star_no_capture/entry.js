import assert from 'node:assert'
import {ns} from './bar'
let foo = 234
assert.equal(ns.foo, 123)
assert.equal(foo, 234)
