import assert from 'node:assert/strict'
import * as ns from './common'
assert.deepEqual(
  ns,
  Object.defineProperty(
    {
      x: 1,
      z: 4,
    },
    Symbol.toStringTag,
    { value: "Module" },
  ),
)
