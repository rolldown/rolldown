import assert from 'node:assert'

const requiredCjsLib = require('./cjs-lib')
assert.strictEqual(requiredCjsLib(), 'exports')
assert.strictEqual(requiredCjsLib.foo, 'foo')

const requiredUmdLib = require('./umd-lib')
assert.strictEqual(requiredUmdLib(), 'exports')
assert.strictEqual(requiredUmdLib.foo, 'foo')
