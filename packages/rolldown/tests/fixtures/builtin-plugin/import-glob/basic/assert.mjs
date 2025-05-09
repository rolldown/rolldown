import assert from 'node:assert'
import { modules1, modules2 } from './dist/main'

modules1['./dir/a.js']().then((m) => {
  assert.strictEqual(m.default, 'a')
})

modules2['./dir/b.ts']().then((m) => {
  assert.strictEqual(m, 'b')
})
