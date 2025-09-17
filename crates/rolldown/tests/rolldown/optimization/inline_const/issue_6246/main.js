import assert from 'node:assert'
import { character, next, immutable_let, mutable_let } from './token.js'

console.log(character)
assert.strictEqual(character, 0)
next()
assert.strictEqual(character, 1)

assert.strictEqual(immutable_let, 'immutable_let')
assert.strictEqual(mutable_let, 'mutable_let1')
