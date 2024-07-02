import { deepEqual } from 'node:assert'

deepEqual(require('demo-pkg'), {
  default: 'module'
})
