import assert from 'node:assert'
import { c, d } from './dist/main'

assert.strictEqual(c, 0)
assert.strictEqual(c, d)
