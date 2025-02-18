// @ts-nocheck
import assert from 'node:assert'
import modules from './dist/main'

assert.strictEqual(Object.keys(modules).length, 2)

modules['../dir/a.js']().then((m) => {
  assert.strictEqual(m.default, 'a')
})

modules['../dir/b.js']().then((m) => {
  assert.strictEqual(m.default, 'b')
})
