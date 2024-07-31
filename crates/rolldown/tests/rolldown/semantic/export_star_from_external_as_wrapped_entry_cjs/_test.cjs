const { readFile: readFile2 } = require('./dist/entry.cjs');
const { readFile } = require('fs');
const assert = require('assert');
assert.strictEqual(readFile, readFile2)
