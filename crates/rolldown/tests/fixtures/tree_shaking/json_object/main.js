import assert from 'node:assert'
import { shouldBeReserved } from './foo.json'
assert.equal(shouldBeReserved, 'shouldBeReserved')
