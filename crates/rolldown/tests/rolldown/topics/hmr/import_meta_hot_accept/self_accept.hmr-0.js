import assert from "node:assert"

export const foo = 'foo2'

import.meta.hot.accept(mod => {
  assert.strictEqual(mod.foo, 'foo2')
})
