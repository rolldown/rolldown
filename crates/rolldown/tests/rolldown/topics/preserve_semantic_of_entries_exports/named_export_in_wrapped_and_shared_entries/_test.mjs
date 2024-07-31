import assert from 'node:assert'
import * as entry from './dist/entry.mjs'
import * as entry2 from './dist/entry2.mjs'
assert.deepStrictEqual(entry.foo, 'foo')
assert.deepStrictEqual(entry.foo, entry2.foo)
assert.deepStrictEqual(entry.default, 'main')
assert.deepStrictEqual(entry.default, entry2.default)