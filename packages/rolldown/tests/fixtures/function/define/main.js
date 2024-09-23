import assert from 'node:assert'

console.log(process.env.NODE_ENV)
assert.strictEqual(process.env.NODE_ENV, 'production')

// It should not define shadowed variables.
;(function (process) {
  assert.strictEqual(process.env.NODE_ENV, undefined)
})({ env: {} })
