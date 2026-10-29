import assert from 'node:assert/strict'

assert.deepEqual(require('demo-pkg'), {
  default: 'module'
})
