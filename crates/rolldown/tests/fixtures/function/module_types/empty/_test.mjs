import assert from 'node:assert'
import { notExistExport } from './dist/main.mjs'
assert.equal(notExistExport, void 0)