import { createRequire } from 'node:module'

const require = createRequire(import.meta.url)

const assert = require('node:assert')

assert.equal(1, 1)
