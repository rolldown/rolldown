import assert from 'node:assert'
import { notExistExport } from './dist/main.js'
assert.equal(notExistExport, void 0)
