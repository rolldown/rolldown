import assert from 'node:assert/strict'
import * as ns from './bar'
let foo = 234
assert.deepEqual(
  ns,
  Object.defineProperty(
    {
      foo: 123,
    },
    Symbol.toStringTag,
    { value: "Module" },
  ),
)
assert.equal(ns.foo, 123)
assert.equal(foo, 234)
