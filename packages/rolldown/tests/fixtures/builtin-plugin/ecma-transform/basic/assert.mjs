// @ts-nocheck
import assert from 'node:assert'
import { a, b } from './dist/main'

assert.strictEqual(a, 1)
assert.strictEqual(b, 2)
