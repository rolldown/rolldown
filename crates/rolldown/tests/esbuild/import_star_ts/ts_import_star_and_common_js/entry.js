import assert from 'node:assert'
import * as ns from './foo'
const ns2 = require('./foo')
assert.equal(ns.foo, 123)
assert.equal(ns2.foo, 123)
