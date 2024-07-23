const x_b64 = require('./x.b64')
import assert from 'node:assert'
import y_b64 from './y.b64'
assert.deepEqual(x_b64, {
  default: 'eA=='
})
assert.equal(y_b64, 'eQ==')
