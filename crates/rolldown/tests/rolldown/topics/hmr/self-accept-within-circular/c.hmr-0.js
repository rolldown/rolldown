import assert from 'node:assert'
import './b'

export const c = 'cc'

assert.strictEqual(c, 'cc')

import.meta.hot.accept((nextExports) => {
  assert.strictEqual(nextExports.c, 'cc')
})
