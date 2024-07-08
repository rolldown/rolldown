const fs = require('node:fs');
const assert = require('node:assert');
const path = require('node:path');
const { parse } = require('cjs-module-lexer');

const parsed = parse(fs.readFileSync(path.resolve(__dirname, 'dist/main.cjs'), 'utf8'));

assert(parsed.exports.length === 0)
assert.deepStrictEqual(parsed.reexports, ['./ext.js'])