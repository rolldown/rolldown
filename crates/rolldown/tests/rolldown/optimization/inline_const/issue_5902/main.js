import { foo, setFoo } from './foo.mjs'
import assert from 'node:assert'


setFoo(20)
assert.strictEqual(foo, 20)
