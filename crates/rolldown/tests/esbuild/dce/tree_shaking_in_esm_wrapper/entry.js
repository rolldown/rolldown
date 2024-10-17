import assert from 'node:assert'
import {keep1} from './lib'
assert.equal(keep1(), "keep1")
assert.deepEqual(require('./cjs'), {default:"keep2"})


