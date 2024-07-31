import assert from 'node:assert'
import { a } from './cjs'
require('./foo')
assert.equal(a, undefined)
