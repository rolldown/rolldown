import assert from 'node:assert'

assert.strictEqual(Promise, 'promise-shim')
assert.strictEqual(P, 'promise-shim')
assert.strictEqual($, 'jquery')
assert.strictEqual(fs.default, 'node-fs')
assert.strictEqual(Object.assign, 'object-assign-shim')
;(function (Promise, P, $, fs, Object) {
  assert.strictEqual(Promise, undefined)
  assert.strictEqual(P, undefined)
  assert.strictEqual($, undefined)
  assert.strictEqual(fs.default, undefined)
  assert.strictEqual(Object.assign, undefined)
})(undefined, undefined, undefined, {}, {})
