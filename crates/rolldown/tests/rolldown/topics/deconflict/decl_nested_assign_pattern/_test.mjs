import assert from 'node:assert'
import * as main from './dist/main.mjs'

assert.equal(main.baz, 'baz')
assert.equal(main.baz2, 'baz2')
