import assert from "node:assert/strict"
import * as ns from "demo-pkg"
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
