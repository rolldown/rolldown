import assert from 'node:assert'
import { a as a2 } from './proxy'
const a = 'index.js'
assert.equal(a, 'index.js')
assert.equal(a2, 'a.js')
