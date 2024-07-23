import assert from 'node:assert'
import * as foo_ns from './foo'
console.log(foo_ns)
assert.deepEqual(foo_ns, {
  bar_ns: {
    bar: 123
  }
})
