import assert from 'node:assert'
import * as foo_ns from './foo'
console.log(foo_ns)
assert.deepEqual(foo_ns, {
  [Symbol.toStringTag]: 'Module',
  bar_ns: {
    [Symbol.toStringTag]: 'Module',
    bar: 123
  }
})
