import assert from 'node:assert'

const requiredCjsLib = require('./cjs-lib')
// hyf0 FIXME: Should support re-assign `module.exports`
// assert.strictEqual(requiredCjsLib(), 'exports')
assert.strictEqual(requiredCjsLib.foo, 'foo')
