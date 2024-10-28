import assert from 'node:assert'
import './a.empty'
import * as ns from './b.empty'
import def from './c.empty'
import { named } from './d.empty'

assert.deepEqual(ns, {
  default: {}
})
assert.deepEqual(def, {})
assert.equal(named, undefined)
