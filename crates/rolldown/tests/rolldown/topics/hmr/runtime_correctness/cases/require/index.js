import assert from 'node:assert'

const requiredCjsLib = require('./cjs-lib')
assert.strictEqual(requiredCjsLib(), 'exports')
assert.strictEqual(requiredCjsLib.foo, 'foo')
assert.strictEqual(requiredCjsLib.bar, 'bar')
