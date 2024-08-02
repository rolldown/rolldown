import assert from 'node:assert'
import { m } from './dist/main.js'

m['./dir/index.js']().then((m) => {
  assert.strictEqual(m.default, 'dir')
  assert.strictEqual(m.value, 1)
})
