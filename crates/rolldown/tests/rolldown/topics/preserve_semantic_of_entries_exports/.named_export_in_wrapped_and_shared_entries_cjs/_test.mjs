const require = (await import('node:module')).createRequire(import.meta.url);
const assert = require('node:assert');
const entry = require('./dist/entry.cjs');
const entry2 = require('./dist/entry2.cjs');
assert.deepStrictEqual(entry.foo, 'foo');
assert.deepStrictEqual(entry.foo, entry2.foo);
assert.deepStrictEqual(entry.default, 'main');
assert.deepStrictEqual(entry.default, entry2.default);

console.log(import.meta.filename, module.exports)