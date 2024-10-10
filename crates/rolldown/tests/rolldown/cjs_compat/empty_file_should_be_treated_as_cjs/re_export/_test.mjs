import assert from 'assert'
import { staImport, defaultImport } from './dist/main.js'

// Since empty files are treated as CJS, importing them is just like import `module.exports = {}`.

assert.deepEqual(staImport, { default: {} })
assert.deepEqual(defaultImport, {})
