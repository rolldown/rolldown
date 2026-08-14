const x_b64 = require('./x.b64')
import assert from 'node:assert/strict'
import y_b64 from './y.b64'
assert.deepEqual(x_b64, 'eA==')
assert.equal(y_b64, 'eQ==')
