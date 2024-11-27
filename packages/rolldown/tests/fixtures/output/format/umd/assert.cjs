// @ts-nocheck
const assert = require('node:assert')
assert(require('./dist/main.cjs').default === 'default')
