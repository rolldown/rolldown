const x_txt = require('./x.txt')
import assert from 'node:assert'
import y_txt from './y.txt'
assert.equal(x_txt, 'x')
assert.equal(y_txt, 'y')
