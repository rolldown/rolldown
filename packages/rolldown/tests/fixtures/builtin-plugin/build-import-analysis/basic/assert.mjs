// @ts-nocheck
import assert from 'node:assert'
import { foo, a, b } from './dist/main'

assert.strictEqual(a, b)
assert.strictEqual(foo, a)
assert.strictEqual(foo, 100)
