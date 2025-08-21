import assert from "node:assert"

export const foo = 'foo'

import.meta.hot.accept(mod => {
  assert.strictEqual(mod.foo, 'foo')
})
