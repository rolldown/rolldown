import assert from 'node:assert'
import * as ns from 'pkg'

assert.deepEqual(ns, {
  foo: 123,
  bar: 'abc'
})
