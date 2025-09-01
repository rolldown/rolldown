import assert from 'node:assert'
import './b'

export const c = 'c'

assert.strictEqual(c, 'c')

import.meta.hot.accept((nextExports) => {
  assert.strictEqual(nextExports.c, 'c')
})
