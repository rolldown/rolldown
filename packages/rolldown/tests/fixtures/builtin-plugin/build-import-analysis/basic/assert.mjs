// @ts-nocheck
import assert from 'node:assert'
import { foo, b } from './dist/main'

assert.strictEqual(foo, 100)
assert.strictEqual(b, 2)
