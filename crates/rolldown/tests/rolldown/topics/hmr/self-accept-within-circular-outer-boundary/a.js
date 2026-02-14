import assert from 'node:assert'
import { b } from './b'

export const a = {
  b,
}

import.meta.hot.accept((nextExports) => {
  assert.strictEqual(nextExports.a.b.c, 'cc')
})
