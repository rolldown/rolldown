import * as foo from './foo/test'
import * as bar from './bar/test'
import assert from 'node:assert'
assert.deepEqual(foo, { foo: 123 })
assert.deepEqual(bar, { bar: 123 })
console.log(exports, module.exports)
