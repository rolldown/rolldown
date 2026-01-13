import assert from 'assert'
import { conflict as conflictA, getA } from './a'
import { conflict as conflictB, getB } from './b'

assert.equal(conflictA, 1)
assert.equal(conflictB, 3)
assert.equal(getA(1), 2)
assert.equal(getB(), 3)