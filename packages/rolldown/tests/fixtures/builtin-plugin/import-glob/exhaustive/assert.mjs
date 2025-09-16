import assert from 'node:assert'
import { modules1, modules2 } from './dist/main'

assert.strictEqual(modules1['./node_modules/a.js'], undefined)
assert.strictEqual(modules1['./.dot/b.ts'], undefined)

modules2['./node_modules/a.js']().then((m) => {
  assert.strictEqual(m.default, 'a')
})

modules2['./.dot/b.ts']().then((m) => {
  assert.strictEqual(m.default, 'b')
})
