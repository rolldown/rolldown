import assert from 'node:assert/strict'
import './a.empty'
import * as ns from './b.empty'
import def from './c.empty'
import { named } from './d.empty'

assert.deepEqual(
  ns,
  Object.defineProperty(
    {},
    Symbol.toStringTag,
    { value: "Module" },
  ),
)
assert.deepEqual(def, undefined)
assert.equal(named, undefined)
