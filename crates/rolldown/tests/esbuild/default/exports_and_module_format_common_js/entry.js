import * as foo from './foo/test'
import * as bar from './bar/test'
import assert from 'node:assert'
console.log(exports, module.exports)
assert.deepEqual(foo, { foo: 123 })
assert.deepEqual(bar, { bar: 123 })
