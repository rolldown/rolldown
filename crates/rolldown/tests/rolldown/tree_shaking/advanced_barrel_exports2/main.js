import assert from 'node:assert';
import * as ns from './shared'

assert.equal(ns.a.b, 500)
assert.equal(ns.a['a'], 100)
