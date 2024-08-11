import assert from 'node:assert'
import { deprecate } from 'node:util'
const exports = require('util-deprecate')
assert.strictEqual(deprecate, exports)