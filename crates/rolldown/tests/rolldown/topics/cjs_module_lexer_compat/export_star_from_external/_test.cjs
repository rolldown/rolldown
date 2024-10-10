const fs = require('node:fs');
const assert = require('node:assert');
const path = require('node:path');
const { parse } = require('cjs-module-lexer');

const parsed = parse(fs.readFileSync(path.resolve(__dirname, 'dist/main.js'), 'utf8'));
parsed.reexports.sort();
assert.deepStrictEqual(parsed, {
  exports: [],
  reexports: ['node:fs', 'node:path'].sort(),
})
