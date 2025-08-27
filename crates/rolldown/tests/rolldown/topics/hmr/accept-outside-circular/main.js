import assert from 'node:assert'
import { a } from './a'

assert.strictEqual(a.b.c, 'c')

import.meta.hot.accept((newMod) => {
  assert.strictEqual(newMod.a.b.c, 'cc')
})
