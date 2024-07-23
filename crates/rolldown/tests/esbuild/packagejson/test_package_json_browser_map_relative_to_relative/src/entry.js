import assert from 'node:assert'
import fn from 'demo-pkg'

assert.deepEqual(fn(), ['main-browser', 'util-browser'])
