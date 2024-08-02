import assert from 'node:assert'
import { m1 } from './dist/main.js'

m1['./dir/index.js']().then((m) => {
  assert.strictEqual(m.default, 'dir')
  assert.strictEqual(m.value, 1)
})
