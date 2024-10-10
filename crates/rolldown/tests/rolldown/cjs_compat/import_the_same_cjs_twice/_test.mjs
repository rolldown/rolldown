import assert from 'node:assert'
import { a, a2 } from './dist/main.js'
assert.equal(a, a2)
