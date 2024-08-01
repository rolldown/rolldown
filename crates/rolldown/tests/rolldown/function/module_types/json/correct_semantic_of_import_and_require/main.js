
import assert from 'node:assert'
import codes from './codes.json'
const codes2 = require('./codes.json')

// Make sure the two has the same reference
assert.strictEqual(codes, codes2)