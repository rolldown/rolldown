import assert from 'node:assert';
import * as ns from './shared'

let q = 'a'
// should include all symbols in `a`
assert.equal(ns.a[q], 100)
