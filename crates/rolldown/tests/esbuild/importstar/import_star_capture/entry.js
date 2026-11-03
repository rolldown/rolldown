import assert from 'node:assert/strict'
import * as ns from './foo'
let foo = 234
assert.deepEqual(
  ns,
  Object.defineProperty(
    {
      __proto__: null,
      foo: 123,
    },
    Symbol.toStringTag,
    { value: "Module" },
  ),
)
assert.equal(ns.foo, 123)
assert.equal(foo, 234)
