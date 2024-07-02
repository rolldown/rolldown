import assert from 'node:assert'
import './a.empty'
import * as ns from './b.empty'
import def from './c.empty'
import { named } from './d.empty'
console.log(ns, def, named)
assert.deepEqual(ns, {})
assert.equal(def, undefined)
assert.equal(named, undefined)
