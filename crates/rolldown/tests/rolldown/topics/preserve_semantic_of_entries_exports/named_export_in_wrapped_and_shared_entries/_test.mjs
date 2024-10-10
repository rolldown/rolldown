import assert from 'node:assert'
import * as entry from './dist/entry.js'
import * as entry2 from './dist/entry2.js'
assert.deepStrictEqual(entry.foo, 'foo')
assert.deepStrictEqual(entry.foo, entry2.foo)
assert.deepStrictEqual(entry.default, 'main')
assert.deepStrictEqual(entry.default, entry2.default)
