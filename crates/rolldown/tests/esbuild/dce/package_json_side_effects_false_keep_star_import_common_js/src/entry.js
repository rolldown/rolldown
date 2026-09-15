import assert from "node:assert/strict"
import * as ns from "demo-pkg"
assert.deepEqual(ns, {
  default: {
    foo: 123
  },
  foo: 123
})
