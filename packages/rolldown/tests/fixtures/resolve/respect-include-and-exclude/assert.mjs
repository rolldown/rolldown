import assert from 'node:assert'
import bar from './dist/bar.js'
import foo from './dist/foo.js'

assert.notStrictEqual(bar, 'bar')
assert.notStrictEqual(foo, 'foo')
