import assert from 'node:assert'
import nodeFs from 'node:fs'
import nodePath from 'node:path'
import { lib } from './dist/main.js'

assert.strictEqual(lib, 'prod')
const bundledFile = nodeFs.readFileSync(nodePath.resolve(import.meta.dirname, 'dist/main.js'))
assert(bundledFile.includes('lib.prod.js'))
assert(!bundledFile.includes('lib.dev.js'))
