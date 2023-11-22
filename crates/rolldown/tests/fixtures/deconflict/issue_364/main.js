import assert from 'node:assert'
import { a as a2 } from './shared'
const a = 'a'
const a$1 = 'a$1'

assert.equal(a2, 'shared.js')
assert.equal(a, 'a')
assert.equal(a$1, 'a$1')
