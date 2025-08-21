import assert from "node:assert"

export const foo = 'foo3'

import.meta.hot.accept(mod => {
  assert.strictEqual(mod.foo, 'foo3')
})
