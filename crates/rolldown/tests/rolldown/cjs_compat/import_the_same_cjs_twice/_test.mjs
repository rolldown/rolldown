import assert from 'node:assert'
import { a, a2 } from './dist/main.mjs'
assert.equal(a, a2)
