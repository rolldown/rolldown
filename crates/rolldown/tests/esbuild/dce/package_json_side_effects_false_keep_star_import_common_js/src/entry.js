import assert from "node:assert"
import * as ns from "demo-pkg"
assert.deepEqual(ns, {
  default: {
    foo: 123
  },
  foo: 123
})
