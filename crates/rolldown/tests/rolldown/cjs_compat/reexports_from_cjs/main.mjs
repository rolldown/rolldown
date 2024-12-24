import assert from 'node:assert'
import { bar } from './reexports.js'
assert.equal(bar, 1)

