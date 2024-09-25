// @ts-nocheck
import assert from 'node:assert'
import { modules1, modules2 } from './dist/main'

modules1['./dir/index.js']().then((m) => {
  assert.strictEqual(m.default, 'dir')
  assert.strictEqual(m.value, 1)
})

modules2['./dir/index.js']().then((m) => {
  assert.strictEqual(m, 1)
})
