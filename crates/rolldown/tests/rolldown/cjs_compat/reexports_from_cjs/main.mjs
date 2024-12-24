import assert from 'node:assert'
import { bar } from './reexports.mjs'
assert.equal(bar, 1)

