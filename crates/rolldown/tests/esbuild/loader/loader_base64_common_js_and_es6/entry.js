const x_b64 = require('./x.b64')
import assert from 'node:assert'
import y_b64 from './y.b64'
assert.equal(x_b64, 'x')
assert.equal(y_b64, 'y')
