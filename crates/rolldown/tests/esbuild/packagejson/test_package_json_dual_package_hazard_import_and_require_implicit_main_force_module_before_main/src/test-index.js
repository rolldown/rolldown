import assert from 'node:assert'

assert.deepEqual(require('demo-pkg'), {
  default: 'module'
})
