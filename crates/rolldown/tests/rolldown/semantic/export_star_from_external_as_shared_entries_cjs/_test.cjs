const { readFile: readFile2 } = require('./dist/entry.cjs')
const { readFile: readFile3 } = require('./dist/entry2.cjs')
const { readFile } = require('node:fs')
const assert = require('node:assert')

assert.strictEqual(readFile, readFile2)
assert.strictEqual(readFile, readFile3)
