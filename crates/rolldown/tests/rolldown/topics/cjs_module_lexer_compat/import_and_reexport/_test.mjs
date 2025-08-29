const require = (await import('node:module')).createRequire(import.meta.url);
const fs = require('node:fs');
const assert = require('node:assert');
const path = require('node:path');
const { parse } = require('cjs-module-lexer');

const parsed = parse(fs.readFileSync(path.resolve(import.meta.dirname, 'dist/main.js'), 'utf8'));
assert.deepStrictEqual(parsed, {
  exports: ['readFileSync'], reexports: ['external']
})
