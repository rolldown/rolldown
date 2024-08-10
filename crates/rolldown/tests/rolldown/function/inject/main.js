import assert from 'node:assert'

assert.strictEqual(Promise, 'promise-shim')
assert.strictEqual(P, 'promise-shim')
assert.strictEqual($, 'jquery')
assert.strictEqual(fs.default, 'node-fs')
// FIXME: oxc injects invalid statements `import { default as 'Object.assign' from 'object-assign-shim'` }`, so it fails.
// assert.strictEqual(Object.assign, 'object-assign-shim')