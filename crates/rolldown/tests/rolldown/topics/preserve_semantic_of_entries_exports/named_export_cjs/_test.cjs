const assert = require('node:assert');
const main = require('./dist/main.js');

assert.deepStrictEqual(main.foo, 'foo');
assert.deepStrictEqual(main.default, 'main');
