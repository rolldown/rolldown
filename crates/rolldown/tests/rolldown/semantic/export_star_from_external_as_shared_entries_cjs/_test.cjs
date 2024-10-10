const { readFile: readFile2 } = require('./dist/entry.js')
const { readFile: readFile3 } = require('./dist/entry2.js')
const { readFile } = require('node:fs')
const assert = require('node:assert')

assert.strictEqual(readFile, readFile2)
assert.strictEqual(readFile, readFile3)
