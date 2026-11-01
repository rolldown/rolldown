import assert from 'node:assert/strict'
import {ns} from './foo'
assert.deepEqual(
  ns,
  Object.defineProperty(
    {
      __proto__: null,
      x: 123,
    },
    Symbol.toStringTag,
    { value: "Module" },
  ),
)
assert.equal(ns.foo, undefined)
