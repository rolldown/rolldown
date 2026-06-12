import assert from 'node:assert/strict'
import * as foo_ns from './foo'
console.log(foo_ns)
assert.deepEqual(
  foo_ns,
  Object.defineProperty(
    {
      bar_ns: Object.defineProperty(
        { bar: 123 },
        Symbol.toStringTag,
        { value: "Module" },
      )
    },
    Symbol.toStringTag,
    { value: "Module" },
  ),
)
