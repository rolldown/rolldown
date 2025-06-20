// @ts-nocheck
import assert from 'node:assert'
import { foo, a, b, c } from './dist/main'

assert.strictEqual(a, b)
assert.strictEqual(b, c)
assert.strictEqual(foo, c)
assert.strictEqual(foo, 100)
