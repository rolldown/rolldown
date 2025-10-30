import assert from 'node:assert'
import Bar from './dist/bar.js'
import Foo from './dist/foo.js'

assert.ok(Bar.toString().includes("b("))
assert.ok(Foo.toString().includes("f("))
