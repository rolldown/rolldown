const assert = require('node:assert');
const { test: barTest } = require('./bar.mjs');
const { default: test } = require('./foo.mjs');

assert.strictEqual(barTest.name, 'test');
assert.strictEqual(test.name, 'test');
