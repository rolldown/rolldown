import assert from "node:assert"
import * as ns from "demo-pkg"
assert.deepEqual(ns, {
  [Symbol.toStringTag]: "Module",
  foo: 123
})
