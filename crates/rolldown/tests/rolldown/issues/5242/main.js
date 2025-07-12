import assert from 'node:assert'
exports.foo = 'foo'

assert.strictEqual(this.foo, 'foo')

