import assert from 'node:assert'

assert.strictEqual(Promise, 'promise-shim')
assert.strictEqual(P, 'promise-shim')
assert.strictEqual($, 'jquery')
assert.strictEqual(fs.default, 'node-fs')
assert.strictEqual(Object.assign, 'object-assign-shim')
