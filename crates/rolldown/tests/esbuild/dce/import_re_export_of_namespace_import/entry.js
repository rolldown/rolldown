import assert from 'node:assert'
import * as ns from 'pkg' // => const import_xxx = require_xxx

assert.equal(ns.foo, 123)
