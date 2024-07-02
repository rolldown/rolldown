const x_txt = require('./x.txt')
import assert from 'node:assert'
import y_txt from './y.txt'
assert.deepEqual(x_txt, {
  default: 'x'
})
assert.equal(y_txt, 'y')
